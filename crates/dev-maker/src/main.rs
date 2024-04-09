use crate::logger::init_tracing;
use crate::orderbook_client::OrderbookClient;
use anyhow::Result;
use clap::Parser;
use commons::NewLimitOrder;
use commons::NewOrder;
use reqwest::Url;
use rust_decimal::Decimal;
use secp256k1::rand;
use secp256k1::PublicKey;
use secp256k1::SecretKey;
use secp256k1::SECP256K1;
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::time::sleep;
use tracing::metadata::LevelFilter;
use trade::ContractSymbol;
use trade::Direction;
use uuid::Uuid;

mod historic_rates;
mod logger;
mod orderbook_client;

const ORDER_EXPIRY: u64 = 30;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(LevelFilter::DEBUG)?;

    let opts: Opts = Opts::parse();

    let rates: Vec<Decimal> = match opts.sub_command() {
        SubCommand::Historic => {
            let mut historic_rates = historic_rates::read();
            historic_rates.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

            historic_rates.into_iter().map(|rate| rate.open).collect()
        }
        SubCommand::Fixed(Fixed { price }) => vec![Decimal::try_from(price)?],
    };

    let client = OrderbookClient::new(Url::from_str("http://localhost:8000")?);
    let secret_key = SecretKey::new(&mut rand::thread_rng());
    let public_key = secret_key.public_key(SECP256K1);

    tracing::info!(pubkey = public_key.to_string(), "Starting new dev-maker");

    let mut past_ids = vec![];
    loop {
        for rate in &rates {
            let mut tmp_ids = vec![];
            for _ in 0..5 {
                tmp_ids.push(
                    post_order(
                        client.clone(),
                        secret_key,
                        public_key,
                        Direction::Short,
                        rate + Decimal::ONE,
                        ORDER_EXPIRY,
                    )
                    .await,
                );
                tmp_ids.push(
                    post_order(
                        client.clone(),
                        secret_key,
                        public_key,
                        Direction::Long,
                        rate - Decimal::ONE,
                        ORDER_EXPIRY,
                    )
                    .await,
                );
            }

            for old_id in &past_ids {
                if let Err(err) = client.delete_order(old_id).await {
                    tracing::error!(
                        "Could not delete old order with id {old_id} because of {err:?}"
                    );
                }
            }

            past_ids.clear();

            past_ids.extend(tmp_ids);

            // we sleep a bit shorter than the last order expires to ensure always having an order
            sleep(Duration::from_secs(ORDER_EXPIRY - 1)).await;
        }
    }
}

/// posts a new order
///
/// Define a `spread` which will be added or subtracted from `historic_rate.open`.
/// Remove it or modify it to get some instant profits :)
async fn post_order(
    client: OrderbookClient,
    secret_key: SecretKey,
    public_key: PublicKey,
    direction: Direction,
    price: Decimal,
    order_expiry_seconds: u64,
) -> Uuid {
    let uuid = Uuid::new_v4();
    if let Err(err) = client
        .post_new_order(
            NewOrder::Limit(NewLimitOrder {
                id: uuid,
                contract_symbol: ContractSymbol::BtcUsd,
                price,
                quantity: Decimal::from(5000),
                trader_id: public_key,
                direction,
                leverage: Decimal::from(2),
                expiry: OffsetDateTime::now_utc()
                    + time::Duration::seconds(order_expiry_seconds as i64),
                stable: false,
            }),
            None,
            secret_key,
        )
        .await
    {
        tracing::error!("Failed posting new order {err:?}");
    }
    uuid
}

#[derive(Parser)]
struct Opts {
    #[clap(subcommand)]
    subcmd: Option<SubCommand>,
}

impl Opts {
    fn sub_command(&self) -> SubCommand {
        self.subcmd
            .clone()
            .unwrap_or(SubCommand::Fixed(Fixed { price: 50_000.0 }))
    }
}

#[derive(Parser, Clone)]
enum SubCommand {
    Historic,
    Fixed(Fixed),
}

#[derive(Parser, Clone)]
struct Fixed {
    #[clap(default_value = "50000.0")]
    price: f32,
}
