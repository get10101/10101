use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Txid;
use dlc_manager::ChannelId;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::events::ClosureReason;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(PartialEq, Debug, Clone)]
pub enum ChannelState {
    Pending,
    Open,
    Closed,
    ForceClosedRemote,
    ForceClosedLocal,
}

#[derive(Debug, Clone, PartialEq)]
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

// We create a shadow copy of the channel as the the ldk channel does not live beyond channel
// closure. The main purpose of this shadow is to track general meta data of the channel relevant
// for reporting purposes.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// The `user_channel_id` is set by 10101 at the time the `Event::HTLCIntercepted` when
    /// we are attempting to create a JIT channel.
    pub user_channel_id: UserChannelId,
    /// Until the `Event::ChannelReady` we do not have a `channel_id`, which is derived from
    /// the funding transaction. We use the `user_channel_id` as identifier over the entirety
    /// of the channel lifecycle.
    pub channel_id: Option<ChannelId>,
    pub inbound: u64,
    pub outbound: u64,
    /// Set at the `Event::ChannelReady`
    pub funding_txid: Option<Txid>,
    pub channel_state: ChannelState,
    /// The counter party of the channel.
    pub counterparty: PublicKey,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    /// This data will be updated once the fee information is available. Please note, that
    /// this costs is not a source of truth and may not reflect the latest data. It is
    /// eventually implied from the on-chain fees of the channel transactions attached to
    /// this model.
    pub costs: u64,
}

impl Channel {
    pub fn new(inbound: u64, outbound: u64, counterparty: PublicKey) -> Self {
        Channel {
            user_channel_id: UserChannelId::new(),
            channel_state: ChannelState::Pending,
            inbound,
            outbound,
            counterparty,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            costs: 0,
            channel_id: None,
            funding_txid: None,
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

    pub fn open_channel(
        channel: Option<Channel>,
        channel_details: ChannelDetails,
    ) -> Result<Channel> {
        let mut channel = match channel {
            Some(channel) => channel,
            None => {
                let user_channel_id =
                    UserChannelId::from(channel_details.user_channel_id).to_string();
                tracing::warn!(%user_channel_id, channel_id = %channel_details.channel_id.to_hex(), public = channel_details.is_public, outbound = channel_details.is_outbound, "Creating a new shadow channel");
                Channel::new(
                    channel_details.inbound_capacity_msat / 1000,
                    0,
                    channel_details.counterparty.node_id,
                )
            }
        };

        tracing::debug!("Updating shadow channel.");
        channel.channel_state = ChannelState::Open;
        channel.funding_txid = channel_details.funding_txo.map(|txo| txo.txid);
        channel.channel_id = Some(channel_details.channel_id);
        channel.updated_at = OffsetDateTime::now_utc();
        Ok(channel)
    }
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
