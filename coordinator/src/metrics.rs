use lazy_static::lazy_static;
use opentelemetry::global;
use opentelemetry::metrics::Counter;
use opentelemetry::metrics::Meter;
use opentelemetry::sdk::export::metrics::aggregation;
use opentelemetry::sdk::metrics::controllers;
use opentelemetry::sdk::metrics::processors;
use opentelemetry::sdk::metrics::selectors;
use opentelemetry_prometheus::PrometheusExporter;
use std::time::Duration;

lazy_static! {
    pub static ref METER: Meter = global::meter("coordinator");
    pub static ref SAMPLE_COUNTER: Counter<u64> = METER
        .u64_counter("a.sample_counter")
        .with_description("Counts things")
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
