use lazy_static::lazy_static;
use opentelemetry::global;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::ObservableGauge;
use opentelemetry::metrics::Unit;
use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::controllers;
use opentelemetry::sdk::metrics::processors;
use opentelemetry::sdk::metrics::selectors;
use opentelemetry_prometheus::PrometheusExporter;
use std::time::Duration;

lazy_static! {
    pub static ref METER: Meter = global::meter("coordinator");

    // channel details metrics
    pub static ref CHANNEL_BALANCE_MSATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("channel_balance_msatoshi")
        .with_description("Current channel balance in msatoshi")
        .with_unit(Unit::new("msats"))
        .init();
    pub static ref CHANNEL_OUTBOUND_CAPACITY_MSATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("channel_outbound_capacity_msatoshi")
        .with_description("Channel outbound capacity in msatoshi")
        .with_unit(Unit::new("msats"))
        .init();
    pub static ref CHANNEL_INBOUND_CAPACITY_MSATOSHI: ObservableGauge<u64> = METER
        .u64_observable_gauge("channel_inbound_capacity_msatoshi")
        .with_description("Channel inbound capacity in msatoshi")
        .with_unit(Unit::new("msats"))
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
