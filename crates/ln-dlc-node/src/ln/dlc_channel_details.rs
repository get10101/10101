use crate::bitcoin_conversion::to_secp_pk_30;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::Channel;
use dlc_manager::DlcChannelId;
use serde::Serialize;
use serde::Serializer;

#[derive(Serialize, Debug)]
pub struct DlcChannelDetails {
    #[serde(serialize_with = "optional_channel_id_as_hex")]
    pub dlc_channel_id: Option<DlcChannelId>,
    #[serde(serialize_with = "pk_as_hex")]
    pub counter_party: PublicKey,
    pub channel_state: ChannelState,
    pub signed_channel_state: Option<SignedChannelState>,
    pub update_idx: Option<u64>,
    pub fee_rate_per_vb: Option<u64>,
    pub funding_txid: Option<String>,
    pub funding_tx_vout: Option<usize>,
    pub closing_txid: Option<String>,
}

#[derive(Serialize, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum SignedChannelState {
    Established,
    SettledOffered,
    SettledReceived,
    SettledAccepted,
    SettledConfirmed,
    Settled,
    RenewOffered,
    RenewAccepted,
    RenewConfirmed,
    RenewFinalized,
    Closing,
    CollaborativeCloseOffered,
    SettledClosing,
}

#[derive(Serialize, Debug, Eq, Ord, PartialOrd, PartialEq)]
pub enum ChannelState {
    Offered,
    Accepted,
    Signed,
    Closing,
    SettledClosing,
    Closed,
    CounterClosed,
    ClosedPunished,
    CollaborativelyClosed,
    FailedAccept,
    FailedSign,
    Cancelled,
}

impl From<Channel> for DlcChannelDetails {
    fn from(channel: Channel) -> Self {
        let (update_idx, state, fee_rate_per_vb, funding_txid, funding_tx_vout, closing_txid) =
            match channel.clone() {
                Channel::Signed(SignedChannel {
                    update_idx,
                    fee_rate_per_vb,
                    fund_tx,
                    fund_output_index,
                    state: dlc_manager::channel::signed_channel::SignedChannelState::CollaborativeCloseOffered {
                        close_tx,
                        ..
                    },
                    ..
                }) => (
                    Some(update_idx),
                    Some(SignedChannelState::CollaborativeCloseOffered),
                    Some(fee_rate_per_vb),
                    Some(fund_tx.txid().to_string()),
                    Some(fund_output_index),
                    Some(close_tx.txid().to_string())),
                Channel::Signed(signed_channel) => (
                    Some(signed_channel.update_idx),
                    Some(SignedChannelState::from(signed_channel.state)),
                    Some(signed_channel.fee_rate_per_vb),
                    Some(signed_channel.fund_tx.txid().to_string()),
                    Some(signed_channel.fund_output_index),
                    None,
                ),
                Channel::CollaborativelyClosed(closed_channel)
                | Channel::Closed(closed_channel)
                | Channel::CounterClosed(closed_channel) => (
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(closed_channel.closing_txid.to_string()),
                ),
                Channel::Closing(closing_channel) => (
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(closing_channel.buffer_transaction.txid().to_string()),
                ),
                _ => (None, None, None, None, None, None),
            };

        DlcChannelDetails {
            dlc_channel_id: Some(channel.get_id()),
            counter_party: to_secp_pk_30(channel.get_counter_party_id()),
            channel_state: ChannelState::from(channel),
            signed_channel_state: state.map(SignedChannelState::from),
            update_idx,
            fee_rate_per_vb,
            funding_txid,
            funding_tx_vout,
            closing_txid,
        }
    }
}

impl From<Channel> for ChannelState {
    fn from(value: Channel) -> Self {
        match value {
            Channel::Offered(_) => ChannelState::Offered,
            Channel::Accepted(_) => ChannelState::Accepted,
            Channel::Signed(_) => ChannelState::Signed,
            Channel::Closing(_) => ChannelState::Closing,
            Channel::SettledClosing(_) => ChannelState::SettledClosing,
            Channel::Closed(_) => ChannelState::Closed,
            Channel::CounterClosed(_) => ChannelState::CounterClosed,
            Channel::ClosedPunished(_) => ChannelState::ClosedPunished,
            Channel::CollaborativelyClosed(_) => ChannelState::CollaborativelyClosed,
            Channel::FailedAccept(_) => ChannelState::FailedAccept,
            Channel::FailedSign(_) => ChannelState::FailedSign,
            Channel::Cancelled(_) => ChannelState::Cancelled,
        }
    }
}

impl From<dlc_manager::channel::signed_channel::SignedChannelState> for SignedChannelState {
    fn from(value: dlc_manager::channel::signed_channel::SignedChannelState) -> Self {
        use dlc_manager::channel::signed_channel::SignedChannelState::*;
        match value {
            Established { .. } => SignedChannelState::Established,
            SettledOffered { .. } => SignedChannelState::SettledOffered,
            SettledReceived { .. } => SignedChannelState::SettledReceived,
            SettledAccepted { .. } => SignedChannelState::SettledAccepted,
            SettledConfirmed { .. } => SignedChannelState::SettledConfirmed,
            Settled { .. } => SignedChannelState::Settled,
            RenewOffered { .. } => SignedChannelState::RenewOffered,
            RenewAccepted { .. } => SignedChannelState::RenewAccepted,
            RenewConfirmed { .. } => SignedChannelState::RenewConfirmed,
            RenewFinalized { .. } => SignedChannelState::RenewFinalized,
            Closing { .. } => SignedChannelState::Closing,
            CollaborativeCloseOffered { .. } => SignedChannelState::CollaborativeCloseOffered,
            SettledClosing { .. } => SignedChannelState::SettledClosing,
        }
    }
}

fn optional_channel_id_as_hex<S>(channel_id: &Option<DlcChannelId>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match channel_id {
        Some(channel_id) => s.serialize_str(&hex::encode(channel_id)),
        None => s.serialize_none(),
    }
}

fn pk_as_hex<S>(pk: &PublicKey, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&pk.to_string())
}
