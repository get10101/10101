use flutter_rust_bridge::frb;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

#[frb]
#[derive(Clone, Debug, Default)]
pub struct Price {
    pub bid: f64,
    pub ask: f64,
}

impl From<trade::Price> for Price {
    fn from(value: trade::Price) -> Self {
        Price {
            bid: value.bid.to_f64().expect("price bid to fit into f64"),
            ask: value.ask.to_f64().expect("price ask to fit into f64"),
        }
    }
}

impl From<Price> for trade::Price {
    fn from(value: Price) -> Self {
        trade::Price {
            bid: Decimal::try_from(value.bid).expect("price bid to fit into Decimal"),
            ask: Decimal::try_from(value.ask).expect("price ask to fit into Decimal"),
        }
    }
}

pub struct ChannelInfo {
    /// The total capacity of the channel as defined by the funding output
    pub channel_capacity: u64,
    pub reserve: Option<u64>,
    pub liquidity_option_id: Option<i32>,
}
