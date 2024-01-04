use crate::db;
use crate::node::storage::NodeStorage;
use crate::node::Node;
use crate::storage::CoordinatorTenTenOneStorage;
use dlc_manager::subchannel::SubChannelState;
use lazy_static::lazy_static;
use lightning::ln::channelmanager::ChannelDetails;
use opentelemetry::global;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::ObservableGauge;
use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::controllers;
use opentelemetry::sdk::metrics::processors;
use opentelemetry::sdk::metrics::selectors;
use opentelemetry::Context;
use opentelemetry::KeyValue;
use opentelemetry_prometheus::PrometheusExporter;
use std::sync::Arc;
use std::time::Duration;
use trade::ContractSymbol;
use trade::Direction;

lazy_static! {
    pub static ref METER: Meter = global::meter("maker");

    // channel details metrics
    pub static ref CHANNEL_BALANCE_SATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("channel_balance_satoshi")
        .with_description("Current channel balance in satoshi")
        .init();
    pub static ref CHANNEL_OUTBOUND_CAPACITY_SATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("channel_outbound_capacity_satoshi")
        .with_description("Channel outbound capacity in satoshi")
        .init();
    pub static ref CHANNEL_INBOUND_CAPACITY_SATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("channel_inbound_capacity_satoshi")
        .with_description("Channel inbound capacity in satoshi")
        .init();
    pub static ref CHANNEL_IS_USABLE: ObservableGauge<u64> = METER
        .u64_observable_gauge("channel_is_usable")
        .with_description("If a channel is usable")
        .init();
    pub static ref DLC_CHANNELS_AMOUNT: ObservableGauge<u64> = METER
        .u64_observable_gauge("dlc_channel_amount")
        .with_description("Number of DLC channels")
        .init();
    pub static ref PUNISHED_DLC_CHANNELS_AMOUNT: ObservableGauge<u64> = METER
        .u64_observable_gauge("punished_dlc_channel_amount")
        .with_description("Number of punished DLC channels")
        .init();

    // general node metrics
    pub static ref CONNECTED_PEERS: ObservableGauge<u64> = METER
        .u64_observable_gauge("node_connected_peers_total")
        .with_description("Total number of connected peers")
        .init();
    pub static ref NODE_BALANCE_SATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("node_balance_satoshi")
        .with_description("Node balance in satoshi")
        .init();

    // position metrics
    pub static ref POSITION_QUANTITY: ObservableGauge<f64> = METER
        .f64_observable_gauge("position_quantity_contracts")
        .with_description("Current open position in contracts")
        .init();
    pub static ref POSITION_MARGIN: ObservableGauge<i64> = METER
        .i64_observable_gauge("position_margin_sats")
        .with_description("Current open position margin in sats")
        .init();
}

pub fn init_meter() -> PrometheusExporter {
    let controller = controllers::basic(processors::factory(
        selectors::simple::histogram([1.0, 2.0, 5.0, 10.0, 20.0, 50.0]),
        aggregation::cumulative_temporality_selector(),
    ))
    .with_collect_period(Duration::from_secs(10))
    .build();

    opentelemetry_prometheus::exporter(controller).init()
}

pub fn collect(node: Node) {
    let cx = opentelemetry::Context::current();
    position_metrics(&cx, &node);

    let inner_node = node.inner;
    if let Ok(dlc_channels) = inner_node.list_sub_channels() {
        let (healthy, unhealthy, close_punished) =
            dlc_channels
                .iter()
                .fold(
                    (0, 0, 0),
                    |(healthy, unhealthy, close_punished), c| match c.state {
                        // these are the healthy channels
                        SubChannelState::Signed(_)
                        | SubChannelState::OffChainClosed
                        | SubChannelState::Closing(_) => (healthy + 1, unhealthy, close_punished),
                        // these are settled already, we don't have to look at them anymore
                        SubChannelState::OnChainClosed | SubChannelState::CounterOnChainClosed => {
                            (healthy, unhealthy, close_punished)
                        }
                        SubChannelState::Offered(_)
                        | SubChannelState::Accepted(_)
                        | SubChannelState::Confirmed(_)
                        | SubChannelState::Finalized(_)
                        | SubChannelState::CloseOffered(_)
                        | SubChannelState::CloseAccepted(_)
                        | SubChannelState::CloseConfirmed(_)
                        | SubChannelState::Rejected => (healthy, unhealthy + 1, close_punished),
                        SubChannelState::ClosedPunished(_) => {
                            (healthy, unhealthy, close_punished + 1)
                        }
                    },
                );
        // healthy
        let key_values = [KeyValue::new("is_healthy", true)];
        DLC_CHANNELS_AMOUNT.observe(&cx, healthy as u64, &key_values);
        // unhealthy
        let key_values = [KeyValue::new("is_healthy", false)];
        DLC_CHANNELS_AMOUNT.observe(&cx, unhealthy as u64, &key_values);
        // punished
        PUNISHED_DLC_CHANNELS_AMOUNT.observe(&cx, close_punished as u64, &[]);
    }
    let channels = inner_node.channel_manager.list_channels();
    channel_metrics(&cx, channels);
    node_metrics(&cx, inner_node);
}

fn position_metrics(cx: &Context, node: &Node) {
    let mut conn = match node.pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get pool connection. Error: {e:?}");
            return;
        }
    };

    let positions = match db::positions::Position::get_all_open_positions(&mut conn) {
        Ok(positions) => positions,
        Err(e) => {
            tracing::error!("Failed to get positions. Error: {e:?}");
            return;
        }
    };

    let mut margin_long = 0;
    let mut margin_short = 0;
    let mut quantity_long = 0.0;
    let mut quantity_short = 0.0;

    // Note: we should filter positions here by BTCUSD once we have multiple contract symbols

    for position in positions {
        debug_assert!(
            position.contract_symbol == ContractSymbol::BtcUsd,
            "We should filter positions here by BTCUSD once we have multiple contract symbols"
        );
        match position.direction {
            Direction::Long => {
                // TODO: fix me: this was meant to be the traders margin
                margin_long += position.coordinator_margin;
                quantity_long += position.quantity;
            }
            Direction::Short => {
                margin_short += position.coordinator_margin;
                quantity_short += position.quantity;
            }
        }
    }
    POSITION_QUANTITY.observe(
        cx,
        quantity_long as f64,
        &[
            KeyValue::new("symbol", "BTCUSD"),
            KeyValue::new("status", "open"),
            KeyValue::new("direction", "long"),
        ],
    );
    POSITION_QUANTITY.observe(
        cx,
        quantity_short as f64,
        &[
            KeyValue::new("symbol", "BTCUSD"),
            KeyValue::new("status", "open"),
            KeyValue::new("direction", "short"),
        ],
    );
    POSITION_MARGIN.observe(
        cx,
        margin_long,
        &[
            KeyValue::new("symbol", "BTCUSD"),
            KeyValue::new("status", "open"),
            KeyValue::new("direction", "long"),
        ],
    );
    POSITION_MARGIN.observe(
        cx,
        margin_short,
        &[
            KeyValue::new("symbol", "BTCUSD"),
            KeyValue::new("status", "open"),
            KeyValue::new("direction", "short"),
        ],
    );
}

fn channel_metrics(cx: &Context, channels: Vec<ChannelDetails>) {
    for channel_detail in channels {
        let key_values = [
            KeyValue::new("channel_id", hex::encode(channel_detail.channel_id.0)),
            KeyValue::new("is_outbound", channel_detail.is_outbound),
            KeyValue::new("is_public", channel_detail.is_public),
        ];
        CHANNEL_BALANCE_SATOSHI.observe(cx, channel_detail.balance_msat / 1_000, &key_values);
        CHANNEL_OUTBOUND_CAPACITY_SATOSHI.observe(
            cx,
            channel_detail.outbound_capacity_msat / 1_000,
            &key_values,
        );
        CHANNEL_INBOUND_CAPACITY_SATOSHI.observe(
            cx,
            channel_detail.inbound_capacity_msat / 1_000,
            &key_values,
        );
        CHANNEL_IS_USABLE.observe(cx, channel_detail.is_usable as u64, &key_values);
    }
}

fn node_metrics(
    cx: &Context,
    inner_node: Arc<ln_dlc_node::node::Node<CoordinatorTenTenOneStorage, NodeStorage>>,
) {
    let connected_peers = inner_node.list_peers().len();
    CONNECTED_PEERS.observe(cx, connected_peers as u64, &[]);
    let offchain = inner_node.get_ldk_balance();

    NODE_BALANCE_SATOSHI.observe(
        cx,
        offchain.available(),
        &[
            KeyValue::new("type", "off-chain"),
            KeyValue::new("status", "available"),
        ],
    );
    NODE_BALANCE_SATOSHI.observe(
        cx,
        offchain.pending_close(),
        &[
            KeyValue::new("type", "off-chain"),
            KeyValue::new("status", "pending_close"),
        ],
    );

    match inner_node.get_on_chain_balance() {
        Ok(onchain) => {
            NODE_BALANCE_SATOSHI.observe(
                cx,
                onchain.confirmed,
                &[
                    KeyValue::new("type", "on-chain"),
                    KeyValue::new("status", "confirmed"),
                ],
            );
            NODE_BALANCE_SATOSHI.observe(
                cx,
                onchain.immature,
                &[
                    KeyValue::new("type", "on-chain"),
                    KeyValue::new("status", "immature"),
                ],
            );
            NODE_BALANCE_SATOSHI.observe(
                cx,
                onchain.trusted_pending,
                &[
                    KeyValue::new("type", "on-chain"),
                    KeyValue::new("status", "trusted_pending"),
                ],
            );
            NODE_BALANCE_SATOSHI.observe(
                cx,
                onchain.untrusted_pending,
                &[
                    KeyValue::new("type", "on-chain"),
                    KeyValue::new("status", "untrusted_pending"),
                ],
            );
        }
        Err(err) => {
            tracing::error!("Could not retrieve on-chain balance for metrics {err:#}")
        }
    }
}
