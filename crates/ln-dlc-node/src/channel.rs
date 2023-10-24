use anyhow::bail;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Txid;
use dlc_manager::ChannelId;
use lightning::events::ClosureReason;
use lightning::ln::channelmanager::ChannelDetails;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;
use time::OffsetDateTime;
use uuid::Uuid;

/// The prefix used in the description field of an JIT channel opening invoice to be paid by the
/// client.
pub const JIT_FEE_INVOICE_DESCRIPTION_PREFIX: &str = "jit-channel-fee-";

/// We introduce a shadow copy of the Lightning channel as LDK deletes channels from its
/// [`ChannelManager`] as soon as they are closed.
///
/// The main purpose of this shadow is to track general metadata of the channel relevant for
/// reporting purposes.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// Custom identifier for a channel which is generated outside of LDK.
    ///
    /// The coordinator sets it after receiving `Event::HTLCIntercepted` as a result of trying to
    /// create a JIT channel.
    ///
    /// The app sets its own when calling `accept_inbound_channel_from_trusted_peer_0conf` when
    /// accepting an inbound JIT channel from the coordinator.
    pub user_channel_id: UserChannelId,
    /// Until the `Event::ChannelReady` we do not have a `channel_id`, which is derived from
    /// the funding transaction. We use the `user_channel_id` as identifier over the entirety
    /// of the channel lifecycle.
    pub channel_id: Option<ChannelId>,
    pub liquidity_option_id: Option<i32>,
    pub inbound_sats: u64,
    pub outbound_sats: u64,
    /// Set at the `Event::ChannelReady`
    pub funding_txid: Option<Txid>,
    pub channel_state: ChannelState,
    /// The counter party of the channel.
    pub counterparty: PublicKey,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Channel {
    pub fn new(
        user_channel_id: UserChannelId,
        inbound_sats: u64,
        outbound_sats: u64,
        counterparty: PublicKey,
    ) -> Self {
        Channel {
            user_channel_id,
            channel_state: ChannelState::Pending,
            inbound_sats,
            outbound_sats,
            counterparty,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            channel_id: None,
            funding_txid: None,
            liquidity_option_id: None,
        }
    }

    pub fn new_jit_channel(
        user_channel_id: UserChannelId,
        counterparty: PublicKey,
        liquidity_option_id: i32,
    ) -> Self {
        Channel {
            user_channel_id,
            channel_state: ChannelState::Announced,
            inbound_sats: 0,
            outbound_sats: 0,
            counterparty,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            channel_id: None,
            funding_txid: None,
            liquidity_option_id: Some(liquidity_option_id),
        }
    }

    pub fn is_closed(&self) -> bool {
        matches!(
            self.channel_state,
            ChannelState::ForceClosedLocal | ChannelState::ForceClosedRemote | ChannelState::Closed
        )
    }

    pub fn close_channel(channel: Channel, reason: ClosureReason) -> Channel {
        if channel.is_closed() {
            tracing::warn!(%channel, "Unexpected state transition. Expected channel state to be either 'Pending' or 'Open', but was '{:?}'", channel.channel_state);
        }

        let mut channel = channel;
        channel.channel_state = reason.into();
        channel.updated_at = OffsetDateTime::now_utc();
        channel
    }

    pub fn update_liquidity(
        channel: &Channel,
        channel_details: &Option<ChannelDetails>,
    ) -> Result<Channel> {
        let mut channel = channel.clone();
        match channel_details {
            Some(channel_details) => {
                channel.inbound_sats = channel_details.inbound_capacity_msat / 1000;
                channel.outbound_sats = channel_details.outbound_capacity_msat / 1000;
                channel.updated_at = OffsetDateTime::now_utc();
                Ok(channel)
            }
            None => {
                bail!("Couldn't find channel details");
            }
        }
    }

    pub fn open_channel(
        channel: Option<Channel>,
        channel_details: ChannelDetails,
    ) -> Result<Channel> {
        let mut channel = match channel {
            Some(channel) => channel,
            None => {
                let user_channel_id = UserChannelId::from(channel_details.user_channel_id);

                tracing::info!(
                    user_channel_id = %user_channel_id.to_string(),
                    channel_id = %channel_details.channel_id.to_hex(),
                    public = channel_details.is_public,
                    outbound = channel_details.is_outbound,
                    "Cannot open non-existent shadow channel. Creating a new one."
                );

                Channel::new(
                    user_channel_id,
                    channel_details.inbound_capacity_msat / 1000,
                    0,
                    channel_details.counterparty.node_id,
                )
            }
        };

        if channel.channel_state != ChannelState::Pending {
            tracing::warn!(%channel, "Opening a channel in state {:?} expected {:?}.", channel.channel_state, ChannelState::Pending);
        }

        // Note: The `ChannelState::OpenUnpaid` will get matched to `ChannelState::Open` for the
        // coordinator as the coordiantor keeps track of the payment itself associated with the
        // channel. The `ChannelState::OpenUnpaid` is used on the app to track whether the opening
        // fees have been paid.
        channel.channel_state = ChannelState::OpenUnpaid;
        channel.funding_txid = channel_details.funding_txo.map(|txo| txo.txid);
        channel.channel_id = Some(channel_details.channel_id);
        channel.updated_at = OffsetDateTime::now_utc();

        tracing::debug!(%channel, "Set shadow channel to open.");

        Ok(channel)
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum ChannelState {
    /// Corresponds to a JIT channel which an app user has registered interest in opening.
    Announced,
    Pending,
    /// Only used by the app to indicate that the open channel fee payment is still pending.
    OpenUnpaid,
    Open,
    Closed,
    ForceClosedRemote,
    ForceClosedLocal,
}

impl From<ClosureReason> for ChannelState {
    fn from(reason: ClosureReason) -> Self {
        match reason {
            ClosureReason::CounterpartyForceClosed { .. }
            | ClosureReason::CommitmentTxConfirmed => ChannelState::ForceClosedRemote,
            ClosureReason::HolderForceClosed { .. } => ChannelState::ForceClosedLocal,
            _ => ChannelState::Closed,
        }
    }
}

/// A custom identifier which we can pass to LDK so that a Lightning channel can be consistently
/// identified throughout its lifetime.
#[derive(Copy, Debug, Clone, PartialEq)]
pub struct UserChannelId(Uuid);

impl Default for UserChannelId {
    fn default() -> Self {
        Self::new()
    }
}

impl UserChannelId {
    pub fn new() -> UserChannelId {
        UserChannelId(Uuid::new_v4())
    }

    pub fn to_u128(&self) -> u128 {
        self.0.as_u128()
    }
}

impl Display for UserChannelId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format!("{}", self.0).fmt(f)
    }
}

impl From<u128> for UserChannelId {
    fn from(value: u128) -> Self {
        UserChannelId(Uuid::from_u128(value))
    }
}

impl TryFrom<String> for UserChannelId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let user_channel_id = Uuid::from_str(value.as_str())?;
        Ok(UserChannelId(user_channel_id))
    }
}

impl Display for Channel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let channel_id = self
            .channel_id
            .map(|c| c.to_hex())
            .unwrap_or("n/a".to_string());
        format!(
            "user_channel_id: {}, channel_id: {channel_id}, channel_state: {:?}, counterparty: {}, funding_txid: {:?}, created_at: {}, updated_at: {}",
            self.user_channel_id, self.channel_state, self.counterparty, self.funding_txid, self.created_at, self.updated_at
        )
        .fmt(f)
    }
}
