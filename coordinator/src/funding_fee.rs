use crate::db;
use crate::decimal_from_f32;
use crate::message::OrderbookMessage;
use crate::to_nearest_hour_in_the_past;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::SignedAmount;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use rust_decimal_macros::dec;
use std::time::Duration;
use time::ext::NumericalDuration;
use time::format_description;
use time::OffsetDateTime;
use tokio::task::block_in_place;
use tokio_cron_scheduler::JobScheduler;
use xxi_node::commons::ContractSymbol;
use xxi_node::commons::Direction;
use xxi_node::commons::Message;

const RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// The funding rate for any position opened before the `end_date`, which remained open through the
/// `end_date`.
#[derive(Clone, Debug)]
pub struct FundingRate {
    /// A positive funding rate indicates that longs pay shorts; a negative funding rate indicates
    /// that shorts pay longs.
    rate: Decimal,
    /// The start date for the funding rate period. This value is only used for informational
    /// purposes.
    ///
    /// The `start_date` is always a whole hour.
    start_date: OffsetDateTime,
    /// The end date for the funding rate period. When the end date has passed, all active
    /// positions that were created before the end date should be charged a funding fee based
    /// on the `rate`.
    ///
    /// The `end_date` is always a whole hour.
    end_date: OffsetDateTime,
}

impl FundingRate {
    pub(crate) fn new(rate: Decimal, start_date: OffsetDateTime, end_date: OffsetDateTime) -> Self {
        let start_date = to_nearest_hour_in_the_past(start_date);
        let end_date = to_nearest_hour_in_the_past(end_date);

        Self {
            rate,
            start_date,
            end_date,
        }
    }

    pub fn rate(&self) -> Decimal {
        self.rate
    }

    pub fn start_date(&self) -> OffsetDateTime {
        self.start_date
    }

    pub fn end_date(&self) -> OffsetDateTime {
        self.end_date
    }
}

/// A record that a funding fee is owed between the coordinator and a trader.
#[derive(Clone, Copy, Debug)]
pub struct FundingFeeEvent {
    pub id: i32,
    /// A positive amount indicates that the trader pays the coordinator; a negative amount
    /// indicates that the coordinator pays the trader.
    pub amount: SignedAmount,
    pub trader_pubkey: PublicKey,
    pub position_id: i32,
    pub due_date: OffsetDateTime,
    pub price: Decimal,
    pub funding_rate: Decimal,
    pub paid_date: Option<OffsetDateTime>,
}

impl From<FundingFeeEvent> for xxi_node::message_handler::FundingFeeEvent {
    fn from(value: FundingFeeEvent) -> Self {
        Self {
            due_date: value.due_date,
            funding_rate: value.funding_rate,
            price: value.price,
            funding_fee: value.amount,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum IndexPriceSource {
    Bitmex,
    /// The index price will be hard-coded for testing.
    Test,
}

pub async fn generate_funding_fee_events_periodically(
    scheduler: &JobScheduler,
    pool: Pool<ConnectionManager<PgConnection>>,
    auth_users_notifier: tokio::sync::mpsc::Sender<OrderbookMessage>,
    schedule: String,
    index_price_source: IndexPriceSource,
) -> Result<()> {
    scheduler
        .add(tokio_cron_scheduler::Job::new(
            schedule.as_str(),
            move |_, _| {
                let mut attempts_left = 10;

                // We want to retry
                while let (Err(e), true) = (
                    generate_funding_fee_events(
                        &pool,
                        index_price_source,
                        auth_users_notifier.clone(),
                    ),
                    attempts_left > 0,
                ) {
                    attempts_left -= 1;

                    tracing::error!(
                        retry_interval = ?RETRY_INTERVAL,
                        attempts_left,
                        "Failed to generate funding fee events: {e:#}. \
                         Trying again"
                    );

                    std::thread::sleep(RETRY_INTERVAL);
                }
            },
        )?)
        .await?;

    scheduler.start().await?;

    Ok(())
}

/// Generate [`FundingFeeEvent`]s for all active positions.
///
/// When called, a [`FundingFeeEvent`] will be generated for an active position if:
///
/// - We can get a [`FundingRate`] that is at most 1 hour old from the DB.
/// - We can get a BitMEX index price for the `end_date` of the [`FundingRate`].
/// - There is no other [`FundingFeeEvent`] in the DB with the same `position_id` and `end_date`.
/// - The position was created _before_ the `end_date` of the [`FundingRate`].
///
/// This function should be safe to retry. Retry should come in handy if the index price is
/// not available.
fn generate_funding_fee_events(
    pool: &Pool<ConnectionManager<PgConnection>>,
    index_price_source: IndexPriceSource,
    auth_users_notifier: tokio::sync::mpsc::Sender<OrderbookMessage>,
) -> Result<()> {
    let mut conn = pool.get()?;

    tracing::debug!("Generating funding fee events");

    let funding_rate = db::funding_rates::get_funding_rate_charged_in_the_last_hour(&mut conn)?;

    let funding_rate = match funding_rate {
        Some(funding_rate) => funding_rate,
        None => {
            tracing::debug!("No current funding rate for this hour");
            return Ok(());
        }
    };

    // TODO: Funding rates should be specific to contract symbols.
    let contract_symbol = ContractSymbol::BtcUsd;

    let index_price = match index_price_source {
        IndexPriceSource::Bitmex => block_in_place(move || {
            let current_index_price =
                get_bitmex_index_price(&contract_symbol, funding_rate.end_date)?;

            anyhow::Ok(current_index_price)
        })?,
        IndexPriceSource::Test => {
            #[cfg(not(debug_assertions))]
            compile_error!("Cannot use a test index price in release mode");

            dec!(50_000)
        }
    };

    if index_price.is_zero() {
        bail!("Cannot generate funding fee events with zero index price");
    }

    // We exclude active positions which were open after this funding period ended.
    let positions = db::positions::Position::get_all_active_positions_open_before(
        &mut conn,
        funding_rate.end_date,
    )?;
    for position in positions {
        let amount = calculate_funding_fee(
            position.quantity,
            funding_rate.rate,
            index_price,
            position.trader_direction,
        );

        if let Some(funding_fee_event) = db::funding_fee_events::insert(
            &mut conn,
            amount,
            position.trader,
            position.id,
            funding_rate.end_date,
            index_price,
            funding_rate.rate,
        )
        .context("Failed to insert funding fee event")?
        {
            block_in_place(|| {
                auth_users_notifier
                    .blocking_send(OrderbookMessage::TraderMessage {
                        trader_id: position.trader,
                        message: Message::FundingFeeEvent(xxi_node::FundingFeeEvent {
                            contract_symbol,
                            contracts: decimal_from_f32(position.quantity),
                            direction: position.trader_direction,
                            price: funding_fee_event.price,
                            fee: funding_fee_event.amount,
                            due_date: funding_fee_event.due_date,
                        }),
                        notification: None,
                    })
                    .map_err(anyhow::Error::new)
                    .context("Could not send pending funding fee event to trader")
            })?;

            tracing::debug!(
                position_id = %position.id,
                trader_pubkey = %position.trader,
                fee_amount = ?amount,
                ?funding_rate,
                "Generated funding fee event"
            );
        }
    }

    anyhow::Ok(())
}

/// Calculate the funding fee.
///
/// We assume that the `index_price` is not zero. Otherwise, the function panics.
fn calculate_funding_fee(
    quantity: f32,
    // Positive means longs pay shorts; negative means shorts pay longs.
    funding_rate: Decimal,
    index_price: Decimal,
    trader_direction: Direction,
) -> SignedAmount {
    // Transform the funding rate from a global perspective (longs and shorts) to a local
    // perspective (the coordinator-trader position).
    let funding_rate = match trader_direction {
        Direction::Long => funding_rate,
        Direction::Short => -funding_rate,
    };

    let quantity = Decimal::try_from(quantity).expect("to fit");

    // E.g. 500 [$] / 20_000 [$/BTC] = 0.025 [BTC]
    let mark_value = quantity / index_price;

    let funding_fee_btc = mark_value * funding_rate;
    let funding_fee_btc = funding_fee_btc
        .round_dp_with_strategy(8, RoundingStrategy::MidpointAwayFromZero)
        .to_f64()
        .expect("to fit");

    SignedAmount::from_btc(funding_fee_btc).expect("to fit")
}

fn get_bitmex_index_price(
    contract_symbol: &ContractSymbol,
    timestamp: OffsetDateTime,
) -> Result<Decimal> {
    let symbol = bitmex_symbol(contract_symbol);

    let time_format = format_description::parse("[year]-[month]-[day] [hour]:[minute]")?;

    // Ideally we get the price indicated by `timestamp`, but if it is not available we are happy to
    // take a price up to 1 minute in the past.
    let start_time = (timestamp - 1.minutes()).format(&time_format)?;
    let end_time = timestamp.format(&time_format)?;

    let mut url = reqwest::Url::parse("https://www.bitmex.com/api/v1/instrument/compositeIndex")?;
    url.query_pairs_mut()
        .append_pair("symbol", &format!(".{symbol}"))
        .append_pair(
            "filter",
            // The `reference` is set to `BMI` to get the _composite_ index.
            &format!("{{\"symbol\": \".{symbol}\", \"startTime\": \"{start_time}\", \"endTime\": \"{end_time}\", \"reference\": \"BMI\"}}"),
        )
        .append_pair("columns", "lastPrice,timestamp,reference")
        // Reversed to get the latest one.
        .append_pair("reverse", "true")
        // Only need one index.
        .append_pair("count", "1");

    let indices = reqwest::blocking::get(url)?.json::<Vec<Index>>()?;
    let index = &indices[0];

    let index_price = Decimal::try_from(index.last_price)?;

    Ok(index_price)
}

fn bitmex_symbol(contract_symbol: &ContractSymbol) -> &str {
    match contract_symbol {
        ContractSymbol::BtcUsd => "BXBT",
    }
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Index {
    #[serde(with = "time::serde::rfc3339")]
    #[serde(rename = "timestamp")]
    _timestamp: OffsetDateTime,
    last_price: f64,
    #[serde(rename = "reference")]
    _reference: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use rust_decimal_macros::dec;

    #[test]
    fn calculate_funding_fee_test() {
        assert_debug_snapshot!(calculate_funding_fee(
            500.0,
            dec!(0.003),
            dec!(20_000),
            Direction::Long
        ));
        assert_debug_snapshot!(calculate_funding_fee(
            500.0,
            dec!(0.003),
            dec!(20_000),
            Direction::Short
        ));
        assert_debug_snapshot!(calculate_funding_fee(
            500.0,
            dec!(-0.003),
            dec!(20_000),
            Direction::Long
        ));
        assert_debug_snapshot!(calculate_funding_fee(
            500.0,
            dec!(-0.003),
            dec!(20_000),
            Direction::Short
        ));
        assert_debug_snapshot!(calculate_funding_fee(
            500.0,
            dec!(0.003),
            dec!(40_000),
            Direction::Long
        ));
        assert_debug_snapshot!(calculate_funding_fee(
            500.0,
            dec!(0.003),
            dec!(40_000),
            Direction::Short
        ));
        assert_debug_snapshot!(calculate_funding_fee(
            100.0,
            dec!(0.003),
            dec!(20_000),
            Direction::Long
        ));
        assert_debug_snapshot!(calculate_funding_fee(
            100.0,
            dec!(0.003),
            dec!(20_000),
            Direction::Short
        ));
    }
}
