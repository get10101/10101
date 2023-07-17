use anyhow::bail;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::events::ClosureReason;
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

// We create a shadow copy of the channel as the the ldk channel does not
// live beyond channel closure. The main purpose of
// this shadow is to track general meta data of the
// channel relevant for reporting purposes.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub id: Option<i32>,
    // The `user_channel_id` is set by 10101 at the time the `Event::HTLCIntercepted` when
    // we are attempting to create a JIT channel.
    pub user_channel_id: String,
    // Until the `Event::ChannelReady` we do not have a `channel_id`, which is derived from
    // the funding transaction. We use the `user_channel_id` as identifier over the entirety
    // of the channel lifecycle.
    pub channel_id: Option<String>,
    pub capacity: i64,
    pub balance: i64,
    // Set at the `Event::ChannelReady`
    pub funding_txid: Option<String>,
    pub channel_state: ChannelState,
    // The counter party of the channel.
    pub counterparty: PublicKey,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    // This data will be updated once the fee information is available. Please note, that
    // this costs is not a source of truth and may not reflect the latest data. It is
    // eventually implied from the on-chain fees of the channel transactions attached to
    // this model.
    pub costs: u64,
}

impl Channel {
    pub fn new(capacity: i64, balance: i64, counterparty: PublicKey) -> Self {
        let user_channel_id = Uuid::new_v4().to_string();
        Channel {
            id: None,
            user_channel_id,
            channel_state: ChannelState::Pending,
            capacity,
            balance,
            counterparty,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            costs: 0,
            channel_id: None,
            funding_txid: None,
        }
    }

    pub fn get_user_channel_id_as_u128(&self) -> u128 {
        Uuid::from_str(&self.user_channel_id)
            .expect("valid uuid")
            .as_u128()
    }

    pub fn parse_user_channel_id(user_channel_id: u128) -> String {
        Uuid::from_u128(user_channel_id).to_string()
    }

    pub fn is_closed(&self) -> bool {
        matches!(
            self.channel_state,
            ChannelState::ForceClosedLocal | ChannelState::ForceClosedRemote | ChannelState::Closed
        )
    }

    pub fn close_channel(channel: Channel, reason: ClosureReason) -> Channel {
        if channel.is_closed() {
            tracing::warn!(channel.channel_id, channel.user_channel_id, "Unexpected state transition. Expected channel state to be either 'Pending' or 'Open', but was '{:?}'", channel.channel_state);
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
                if channel_details.is_outbound {
                    bail!("Could not find shadow channel");
                } else {
                    tracing::info!("Creating a new shadow channel for inbound channel.");
                    Channel::new(
                        (channel_details.inbound_capacity_msat / 1000) as i64,
                        0,
                        channel_details.counterparty.node_id,
                    )
                }
            }
        };

        tracing::debug!("Updating shadow channel.");
        channel.channel_state = ChannelState::Open;
        channel.funding_txid = channel_details.funding_txo.map(|txo| txo.txid.to_string());
        channel.channel_id = Some(hex::encode(channel_details.channel_id));
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
