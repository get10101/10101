use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::DlcChannelId;
use lightning::ln::ChannelId;
use serde::Serialize;
use serde::Serializer;

#[derive(Serialize, Debug)]
pub struct DlcChannelDetails {
    #[serde(serialize_with = "optional_channel_id_as_hex")]
    pub dlc_channel_id: Option<DlcChannelId>,
    #[serde(serialize_with = "pk_as_hex")]
    pub counter_party: PublicKey,
    pub update_idx: u64,
    pub subchannel_state: SignedChannelState,
    pub fee_rate_per_vb: u64,
}

#[derive(Serialize, Debug)]
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
}

impl From<SignedChannel> for DlcChannelDetails {
    fn from(sc: SignedChannel) -> Self {
        DlcChannelDetails {
            dlc_channel_id: Some(sc.channel_id),
            counter_party: sc.counter_party,
            update_idx: sc.update_idx,
            subchannel_state: SignedChannelState::from(sc.state),
            fee_rate_per_vb: sc.fee_rate_per_vb,
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
        }
    }
}

fn optional_channel_id_as_hex<S>(channel_id: &Option<DlcChannelId>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match channel_id {
        Some(channel_id) => s.serialize_str(&channel_id.to_hex()),
        None => s.serialize_none(),
    }
}

fn pk_as_hex<S>(pk: &PublicKey, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&pk.to_hex())
}
