use secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub struct RouteHintHop {
    pub src_node_id: PublicKey,
    pub short_channel_id: u64,
    pub fees: RoutingFees,
    pub cltv_expiry_delta: u16,
    pub htlc_minimum_msat: Option<u64>,
    pub htlc_maximum_msat: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct RoutingFees {
    pub base_msat: u32,
    pub proportional_millionths: u32,
}

impl From<lightning::routing::router::RouteHintHop> for RouteHintHop {
    fn from(value: lightning::routing::router::RouteHintHop) -> Self {
        Self {
            src_node_id: value.src_node_id,
            short_channel_id: value.short_channel_id,
            fees: value.fees.into(),
            cltv_expiry_delta: value.cltv_expiry_delta,
            htlc_minimum_msat: value.htlc_minimum_msat,
            htlc_maximum_msat: value.htlc_maximum_msat,
        }
    }
}

impl From<lightning::routing::gossip::RoutingFees> for RoutingFees {
    fn from(value: lightning::routing::gossip::RoutingFees) -> Self {
        Self {
            base_msat: value.base_msat,
            proportional_millionths: value.proportional_millionths,
        }
    }
}

impl From<RouteHintHop> for lightning::routing::router::RouteHintHop {
    fn from(value: RouteHintHop) -> Self {
        Self {
            src_node_id: value.src_node_id,
            short_channel_id: value.short_channel_id,
            fees: value.fees.into(),
            cltv_expiry_delta: value.cltv_expiry_delta,
            htlc_minimum_msat: value.htlc_minimum_msat,
            htlc_maximum_msat: value.htlc_maximum_msat,
        }
    }
}

impl From<RoutingFees> for lightning::routing::gossip::RoutingFees {
    fn from(value: RoutingFees) -> Self {
        Self {
            base_msat: value.base_msat,
            proportional_millionths: value.proportional_millionths,
        }
    }
}
