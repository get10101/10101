use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use lightning::chain::transaction::OutPoint;
use lightning::ln::features::ChannelTypeFeatures;
use serde::Serialize;
use serde::Serializer;

#[derive(Serialize, Debug)]
pub struct ChannelDetails {
    #[serde(serialize_with = "channel_id_as_hex")]
    pub channel_id: [u8; 32],
    #[serde(serialize_with = "pk_as_hex")]
    pub counterparty: PublicKey,
    #[serde(serialize_with = "optional_outpoint_to_str")]
    pub funding_txo: Option<OutPoint>,
    #[serde(serialize_with = "optional_outpoint_to_str")]
    pub original_funding_txo: Option<OutPoint>,
    #[serde(serialize_with = "optional_channel_type_to_str")]
    pub channel_type: Option<ChannelTypeFeatures>,
    pub channel_value_satoshis: u64,
    pub unspendable_punishment_reserve: Option<u64>,
    pub user_channel_id: u128,
    pub balance_msat: u64,
    pub outbound_capacity_msat: u64,
    pub next_outbound_htlc_limit_msat: u64,
    pub inbound_capacity_msat: u64,
    pub confirmations_required: Option<u32>,
    pub force_close_spend_delay: Option<u16>,
    pub is_outbound: bool,
    pub is_channel_ready: bool,
    pub is_usable: bool,
    pub is_public: bool,
    pub inbound_htlc_minimum_msat: Option<u64>,
    pub inbound_htlc_maximum_msat: Option<u64>,
    pub config: Option<ChannelConfig>,
}

#[derive(Serialize, Debug)]
pub struct ChannelConfig {
    pub forwarding_fee_proportional_millionths: u32,
    pub forwarding_fee_base_msat: u32,
    pub cltv_expiry_delta: u16,
    pub max_dust_htlc_exposure_msat: u64,
    pub force_close_avoidance_max_fee_satoshis: u64,
}

impl From<lightning::ln::channelmanager::ChannelDetails> for ChannelDetails {
    fn from(cd: lightning::ln::channelmanager::ChannelDetails) -> Self {
        ChannelDetails {
            channel_id: cd.channel_id,
            counterparty: cd.counterparty.node_id,
            funding_txo: cd.funding_txo,
            original_funding_txo: cd.original_funding_outpoint,
            channel_type: cd.channel_type,
            channel_value_satoshis: cd.channel_value_satoshis,
            unspendable_punishment_reserve: cd.unspendable_punishment_reserve,
            user_channel_id: cd.user_channel_id,
            balance_msat: cd.balance_msat,
            outbound_capacity_msat: cd.outbound_capacity_msat,
            next_outbound_htlc_limit_msat: cd.next_outbound_htlc_limit_msat,
            inbound_capacity_msat: cd.inbound_capacity_msat,
            confirmations_required: cd.confirmations_required,
            force_close_spend_delay: cd.force_close_spend_delay,
            is_outbound: cd.is_outbound,
            is_channel_ready: cd.is_channel_ready,
            is_usable: cd.is_usable,
            is_public: cd.is_public,
            inbound_htlc_minimum_msat: cd.inbound_htlc_minimum_msat,
            inbound_htlc_maximum_msat: cd.inbound_htlc_maximum_msat,
            config: cd.config.map(|c| ChannelConfig {
                forwarding_fee_proportional_millionths: c.forwarding_fee_proportional_millionths,
                forwarding_fee_base_msat: c.forwarding_fee_base_msat,
                cltv_expiry_delta: c.cltv_expiry_delta,
                max_dust_htlc_exposure_msat: c.max_dust_htlc_exposure_msat,
                force_close_avoidance_max_fee_satoshis: c.force_close_avoidance_max_fee_satoshis,
            }),
        }
    }
}

fn channel_id_as_hex<S>(channel_id: &[u8; 32], s: S) -> Result<S::Ok, S::Error>
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

fn optional_outpoint_to_str<S>(outpoint: &Option<OutPoint>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match outpoint {
        Some(outpoint) => {
            let txid = outpoint.txid;
            let index = outpoint.index;

            s.serialize_some(&format!("{txid}:{index}"))
        }

        None => s.serialize_none(),
    }
}

fn optional_channel_type_to_str<S>(
    type_features: &Option<ChannelTypeFeatures>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match type_features {
        Some(type_features) => s.serialize_some(&type_features.to_string()),
        None => s.serialize_none(),
    }
}
