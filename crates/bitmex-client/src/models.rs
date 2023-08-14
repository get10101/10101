use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    pub fn to_url(&self) -> String {
        match self {
            Network::Mainnet => "https://www.bitmex.com/api/v1".to_string(),
            Network::Testnet => "https://testnet.bitmex.com/api/v1".to_string(),
        }
    }
}

pub trait Request: Serialize {
    const METHOD: Method;
    const SIGNED: bool = false;
    const ENDPOINT: &'static str;
    const HAS_PAYLOAD: bool = true;
    type Response: DeserializeOwned;

    #[inline]
    fn no_payload(&self) -> bool {
        !Self::HAS_PAYLOAD
    }
}

/// Placement, Cancellation, Amending, and History
#[derive(Clone, Debug, Deserialize)]
pub struct Order {
    #[serde(rename = "orderID")]
    pub order_id: Uuid,
    pub account: Option<i64>,
    pub symbol: Option<String>,
    pub side: Option<Side>,
    #[serde(rename = "orderQty")]
    pub order_qty: Option<i64>,
    pub price: Option<f64>,
    #[serde(rename = "displayQty")]
    pub display_qty: Option<i64>,
    #[serde(rename = "pegPriceType")]
    pub peg_price_type: Option<PegPriceType>,
    #[serde(rename = "ordType")]
    pub ord_type: Option<OrdType>,
    #[serde(rename = "ordStatus")]
    pub ord_status: Option<OrderStatus>,
    pub text: Option<String>,
    #[serde(rename = "transactTime", with = "time::serde::rfc3339::option")]
    pub transact_time: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub timestamp: Option<OffsetDateTime>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
    #[serde(rename = "")]
    Unknown, // BitMEX sometimes has empty side due to unknown reason
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum OrderStatus {
    Filled,
    Open,
    New,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum ExecType {
    Funding,
    Trade,
    #[serde(other)]
    Unknown,
}

/// http://fixwiki.org/fixwiki/PegPriceType
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum PegPriceType {
    LastPeg,
    OpeningPeg,
    MidPricePeg,
    MarketPeg,
    PrimaryPeg,
    PegToVWAP,
    TrailingStopPeg,
    PegToLimitPrice,
    ShortSaleMinPricePeg,
    #[serde(rename = "")]
    Unknown, // BitMEX sometimes has empty due to unknown reason
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum OrdType {
    Market,
    Limit,
    Stop,
    StopLimit,
    MarketIfTouched,
    LimitIfTouched,
    MarketWithLeftOverAsLimit,
    Pegged,
}

/// https://www.onixs.biz/fix-dictionary/5.0.SP2/tagNum_59.html
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum TimeInForce {
    Day,
    GoodTillCancel,
    AtTheOpening,
    ImmediateOrCancel,
    FillOrKill,
    GoodTillCrossing,
    GoodTillDate,
    AtTheClose,
    GoodThroughCrossing,
    AtCrossing,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ExecInst {
    ParticipateDoNotInitiate,
    AllOrNone,
    MarkPrice,
    IndexPrice,
    LastPrice,
    Close,
    ReduceOnly,
    Fixed,
    #[serde(rename = "")]
    Unknown, // BitMEX sometimes has empty due to unknown reason
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ContingencyType {
    OneCancelsTheOther,
    OneTriggersTheOther,
    OneUpdatesTheOtherAbsolute,
    OneUpdatesTheOtherProportional,
    #[serde(rename = "")]
    Unknown, // BitMEX sometimes has empty due to unknown reason
}

/// Create a new order.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PostOrderRequest {
    /// Instrument symbol. e.g. 'XBTUSD'.
    pub symbol: ContractSymbol,
    /// Order side. Valid options: Buy, Sell. Defaults to 'Buy' unless `orderQty` is negative.
    pub side: Option<Side>,
    /// Order quantity in units of the instrument (i.e. contracts).
    #[serde(rename = "orderQty", skip_serializing_if = "Option::is_none")]
    pub order_qty: Option<i32>,
    /// Order type. Valid options: Market, Limit, Stop, StopLimit, MarketIfTouched, LimitIfTouched,
    /// Pegged. Defaults to 'Limit' when `price` is specified. Defaults to 'Stop' when `stopPx` is
    /// specified. Defaults to 'StopLimit' when `price` and `stopPx` are specified.
    #[serde(rename = "ordType", skip_serializing_if = "Option::is_none")]
    pub ord_type: Option<OrdType>,
    /// Optional order annotation. e.g. 'Take profit'.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl Request for PostOrderRequest {
    const METHOD: Method = Method::POST;
    const SIGNED: bool = true;
    const ENDPOINT: &'static str = "/order";
    const HAS_PAYLOAD: bool = true;
    type Response = Order;
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum ContractSymbol {
    #[serde(rename = "XBTUSD")]
    XbtUsd,
}

/// Get your positions.
#[derive(Clone, Debug, Serialize, Default)]
pub struct GetPositionRequest;

impl Request for GetPositionRequest {
    const METHOD: Method = Method::GET;
    const SIGNED: bool = true;
    const ENDPOINT: &'static str = "/position";
    const HAS_PAYLOAD: bool = true;
    type Response = Vec<Position>;
}

/// Summary of Open and Closed Positions
#[derive(Clone, Debug, Deserialize)]
pub struct Position {
    pub account: i64,
    pub symbol: ContractSymbol,
    pub currency: String,
    pub underlying: Option<String>,
    #[serde(rename = "quoteCurrency")]
    pub quote_currency: Option<String>,
    pub leverage: Option<f64>,
    #[serde(rename = "crossMargin")]
    pub cross_margin: Option<bool>,
    #[serde(rename = "currentQty")]
    pub current_qty: Option<i64>,
    #[serde(rename = "maintMargin")]
    pub maint_margin: Option<i64>,
    #[serde(rename = "unrealisedPnl")]
    pub unrealised_pnl: Option<i64>,
    #[serde(rename = "liquidationPrice")]
    pub liquidation_price: Option<f64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub timestamp: Option<OffsetDateTime>,
}
