use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use time::OffsetDateTime;

#[derive(Deserialize, Debug)]
pub(crate) struct HistoricRate {
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    pub open: Decimal,
}

pub fn read() -> Vec<HistoricRate> {
    let file = File::open("./crates/dev-maker/bitmex_hourly_rates.json")
        .expect("To be able to find this file");

    let reader = BufReader::new(file);

    serde_json::from_reader(reader).expect("to be able to deserialize from json")
}
