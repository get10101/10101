use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::subchannel::SubChannel;
use dlc_manager::ChannelId;
use serde::Serialize;
use serde::Serializer;

#[derive(Serialize, Debug)]
pub struct DlcChannelDetails {
    #[serde(serialize_with = "channel_id_as_hex")]
    pub channel_id: ChannelId,
    #[serde(serialize_with = "pk_as_hex")]
    pub counter_party: PublicKey,
    pub update_idx: u64,
    pub state: SubChannelState,
    pub fee_rate_per_vb: u64,
    pub fund_value_satoshis: u64,
    /// Whether the local party is the one who offered the sub channel.
    pub is_offer: bool,
}

#[derive(Serialize, Debug)]
pub enum SubChannelState {
    Offered,
    Accepted,
    Signed,
    Closing,
    OnChainClosed,
    CounterOnChainClosed,
    CloseOffered,
    CloseAccepted,
    CloseConfirmed,
    OffChainClosed,
    ClosedPunished,
    Confirmed,
    Rejected,
}

impl From<SubChannel> for DlcChannelDetails {
    fn from(sc: SubChannel) -> Self {
        DlcChannelDetails {
            channel_id: sc.channel_id,
            counter_party: sc.counter_party,
            update_idx: sc.update_idx,
            state: SubChannelState::from(sc.state),
            fee_rate_per_vb: sc.fee_rate_per_vb,
            fund_value_satoshis: sc.fund_value_satoshis,
            is_offer: sc.is_offer,
        }
    }
}

impl From<dlc_manager::subchannel::SubChannelState> for SubChannelState {
    fn from(value: dlc_manager::subchannel::SubChannelState) -> Self {
        use dlc_manager::subchannel::SubChannelState::*;
        match value {
            Offered(_) => SubChannelState::Offered,
            Accepted(_) => SubChannelState::Accepted,
            Signed(_) => SubChannelState::Signed,
            Closing(_) => SubChannelState::Closing,
            OnChainClosed => SubChannelState::OnChainClosed,
            CounterOnChainClosed => SubChannelState::CounterOnChainClosed,
            CloseOffered(_) => SubChannelState::CloseOffered,
            CloseAccepted(_) => SubChannelState::CloseAccepted,
            CloseConfirmed(_) => SubChannelState::CloseConfirmed,
            OffChainClosed => SubChannelState::OffChainClosed,
            ClosedPunished(_) => SubChannelState::ClosedPunished,
            Confirmed(_) => SubChannelState::Confirmed,
            Rejected => SubChannelState::Rejected,
        }
    }
}

fn channel_id_as_hex<S>(channel_id: &ChannelId, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&hex::encode(channel_id))
}

fn pk_as_hex<S>(pk: &PublicKey, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&pk.to_hex())
}
