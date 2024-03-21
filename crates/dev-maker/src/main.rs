use crate::logger::init_tracing;
use crate::orderbook_client::OrderbookClient;
use anyhow::Result;
use commons::NewOrder;
use commons::OrderType;
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

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(LevelFilter::DEBUG)?;

    let client = OrderbookClient::new(Url::from_str("http://localhost:8000")?);
    let secret_key = SecretKey::new(&mut rand::thread_rng());
    let public_key = secret_key.public_key(SECP256K1);

    tracing::info!(pubkey = public_key.to_string(), "Starting new dev-maker");

    let mut historic_rates = historic_rates::read();
    historic_rates.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut past_ids = vec![];
    loop {
        for historic_rate in &historic_rates {
            let mut tmp_ids = vec![];
            for _ in 0..5 {
                tmp_ids.push(
                    post_order(
                        client.clone(),
                        secret_key,
                        public_key,
                        Direction::Short,
                        historic_rate.open + Decimal::from(1),
                    )
                    .await,
                );
                tmp_ids.push(
                    post_order(
                        client.clone(),
                        secret_key,
                        public_key,
                        Direction::Long,
                        historic_rate.open - Decimal::from(1),
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

            sleep(Duration::from_secs(60)).await;
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
) -> Uuid {
    let uuid = Uuid::new_v4();
    if let Err(err) = client
        .post_new_order(
            NewOrder {
                id: uuid,
                contract_symbol: ContractSymbol::BtcUsd,
                price,
                quantity: Decimal::from(5000),
                trader_id: public_key,
                direction,
                leverage: Decimal::from(2),
                order_type: OrderType::Limit,
                expiry: OffsetDateTime::now_utc() + time::Duration::minutes(1),
                stable: false,
            },
            None,
            secret_key,
        )
        .await
    {
        tracing::error!("Failed posting new order {err:?}");
    }
    uuid
}
