use lazy_static::lazy_static;
use lightning::ln::channelmanager::ChannelDetails;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::node::Node;
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

lazy_static! {
    pub static ref METER: Meter = global::meter("coordinator");

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

    // general node metrics
    pub static ref CONNECTED_PEERS: ObservableGauge<u64> = METER
        .u64_observable_gauge("node_connected_peers_total")
        .with_description("Total number of connected peers")
        .init();
    pub static ref NODE_BALANCE_SATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("node_balance_satoshi")
        .with_description("Node balance in satoshi")
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

pub fn collect(node: Arc<Node<InMemoryStore>>) {
    let cx = opentelemetry::Context::current();

    let channels = node.channel_manager.list_channels();
    channel_metrics(&cx, channels);
    node_metrics(&cx, node);
}

fn channel_metrics(cx: &Context, channels: Vec<ChannelDetails>) {
    for channel_detail in channels {
        let key_values = [
            KeyValue::new("channel_id", hex::encode(channel_detail.channel_id)),
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

fn node_metrics(cx: &Context, node: Arc<Node<InMemoryStore>>) {
    let connected_peers = node.list_peers().len();
    CONNECTED_PEERS.observe(cx, connected_peers as u64, &[]);
    let offchain = node.get_ldk_balance();

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

    match node.get_on_chain_balance() {
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
