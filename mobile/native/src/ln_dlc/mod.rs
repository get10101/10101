use self::node::WalletHistories;
use crate::api;
use crate::calculations;
use crate::config;
use crate::event;
use crate::event::EventInternal;
use crate::ln_dlc::node::Node;
use crate::ln_dlc::node::Payments;
use crate::trade::order::FailureReason;
use crate::trade::position;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bdk::bitcoin::secp256k1::rand::thread_rng;
use bdk::bitcoin::secp256k1::rand::RngCore;
use bdk::bitcoin::secp256k1::SecretKey;
use bdk::bitcoin::XOnlyPublicKey;
use bdk::BlockTime;
use coordinator_commons::TradeParams;
use itertools::chain;
use itertools::Itertools;
use lightning_invoice::Invoice;
use ln_dlc_node::node::NodeInfo;
use ln_dlc_node::seed::Bip39Seed;
use rust_decimal::Decimal;
use state::Storage;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::runtime::Runtime;

mod node;

static NODE: Storage<Arc<Node>> = Storage::new();
const PROCESS_INCOMING_MESSAGES_INTERVAL: Duration = Duration::from_secs(5);

pub async fn refresh_wallet_info() -> Result<()> {
    keep_wallet_balance_and_history_up_to_date(NODE.get()).await?;

    Ok(())
}

pub fn get_seed_phrase() -> Vec<String> {
    NODE.get().get_seed_phrase()
}

pub fn get_node_key() -> Result<SecretKey> {
    NODE.get().inner.node_key()
}

pub fn get_node_info() -> NodeInfo {
    NODE.get().inner.info
}

// TODO: should we also wrap the oracle as `NodeInfo`. It would fit the required attributes pubkey
// and address.
pub fn get_oracle_pubkey() -> XOnlyPublicKey {
    NODE.get().inner.oracle_pk()
}

/// Lazily creates a multi threaded runtime with the the number of worker threads corresponding to
/// the number of available cores.
fn runtime() -> Result<&'static Runtime> {
    static RUNTIME: Storage<Runtime> = Storage::new();

    if RUNTIME.try_get().is_none() {
        let runtime = Runtime::new()?;
        RUNTIME.set(runtime);
    }

    Ok(RUNTIME.get())
}

pub fn run(data_dir: String) -> Result<()> {
    let network = config::get_network();
    let runtime = runtime()?;

    runtime.block_on(async move {
        event::publish(&EventInternal::Init("Starting full ldk node".to_string()));

        let mut ephemeral_randomness = [0; 32];
        thread_rng().fill_bytes(&mut ephemeral_randomness);

        let data_dir = Path::new(&data_dir).join(network.to_string());
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .context(format!("Could not create data dir for {network}"))?;
        }

        event::subscribe(position::subscriber::Subscriber {});
        // TODO: Subscribe to events from the orderbook and publish OrderFilledWith event

        let address = {
            let listener = TcpListener::bind("0.0.0.0:0")?;
            listener.local_addr().expect("To get a free local address")
        };

        let seed_path = data_dir.join("seed");
        let seed = Bip39Seed::initialize(&seed_path)?;

        let node = Arc::new(
            ln_dlc_node::node::Node::new_app(
                "10101",
                network,
                data_dir.as_path(),
                Payments,
                address,
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), address.port()),
                config::get_electrs_endpoint().to_string(),
                seed,
                ephemeral_randomness,
            )
            .await?,
        );
        let node = Arc::new(Node { inner: node });

        runtime.spawn({
            let node = node.clone();
            async move { node.keep_connected(config::get_coordinator_info()).await }
        });

        runtime.spawn({
            let node = node.clone();
            async move {
                loop {
                    node.process_incoming_dlc_messages();

                    tokio::time::sleep(PROCESS_INCOMING_MESSAGES_INTERVAL).await;
                }
            }
        });

        runtime.spawn({
            let node = node.clone();
            async move {
                loop {
                    if let Err(e) = keep_wallet_balance_and_history_up_to_date(&node).await {
                        tracing::error!("Failed to sync balance and wallet history: {e:#}");
                    }

                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        });

        NODE.set(node);

        Ok(())
    })
}

async fn keep_wallet_balance_and_history_up_to_date(node: &Node) -> Result<()> {
    node.inner.sync()?;

    let wallet_balances = node
        .get_wallet_balances()
        .context("Failed to get wallet balances")?;

    let WalletHistories {
        on_chain,
        off_chain,
    } = node
        .get_wallet_histories()
        .context("Failed to get wallet histories")?;

    let on_chain = on_chain.iter().map(|details| {
        let net_sats = details.received as i64 - details.sent as i64;

        let (flow, amount_sats) = if net_sats >= 0 {
            (api::PaymentFlow::Outbound, net_sats as u64)
        } else {
            (api::PaymentFlow::Inbound, net_sats.unsigned_abs())
        };

        let (status, timestamp) = match details.confirmation_time {
            Some(BlockTime { timestamp, .. }) => (api::Status::Confirmed, timestamp),

            None => {
                (
                    api::Status::Pending,
                    // Unconfirmed transactions should appear towards the top of the history
                    OffsetDateTime::now_utc().unix_timestamp() as u64,
                )
            }
        };

        let wallet_type = api::WalletType::OnChain {
            txid: details.txid.to_string(),
        };

        api::WalletHistoryItem {
            flow,
            amount_sats,
            timestamp,
            status,
            wallet_type,
        }
    });

    let off_chain = off_chain.iter().filter_map(|details| {
        tracing::info!(?details, "Off-chain payment details");

        let amount_sats = match details.amount_msat {
            Some(msat) => msat / 1_000,
            // Skip payments that don't yet have an amount associated
            None => return None,
        };

        let status = match details.status {
            ln_dlc_node::node::HTLCStatus::Pending => api::Status::Pending,
            ln_dlc_node::node::HTLCStatus::Succeeded => api::Status::Confirmed,
            // TODO: Handle failed payments
            ln_dlc_node::node::HTLCStatus::Failed => return None,
        };

        let flow = match details.flow {
            ln_dlc_node::PaymentFlow::Inbound => api::PaymentFlow::Inbound,
            ln_dlc_node::PaymentFlow::Outbound => api::PaymentFlow::Outbound,
        };

        let timestamp = details.timestamp.unix_timestamp() as u64;

        let wallet_type = api::WalletType::Lightning {
            payment_hash: hex::encode(details.payment_hash.0),
        };

        Some(api::WalletHistoryItem {
            flow,
            amount_sats,
            timestamp,
            status,
            wallet_type,
        })
    });

    let mut trades = vec![];
    let orders =
        crate::db::get_filled_orders().context("Failed to get filled orders; skipping update")?;

    let mut open_order = None;
    for (i, order) in orders.into_iter().enumerate() {
        // this works because we currently only open and close a position.
        // that means the second one is always the closing (inbound) transaction.
        // note: this logic might not work in case of liquidation
        let (flow, amount_sats) = if i % 2 == 0 {
            open_order = Some(order);
            (
                api::PaymentFlow::Outbound,
                order
                    .trader_margin()
                    .expect("Filled order to have a margin"),
            )
        } else {
            let open_order = open_order.expect("export open order");
            let trader_margin = open_order
                .trader_margin()
                .expect("Filled order to have a margin");
            let execution_price = Decimal::try_from(
                order
                    .execution_price()
                    .expect("execution price to be set on a filled order"),
            )?;

            let opening_price = open_order
                .execution_price()
                .expect("initial execution price to be set on a filled order");

            let pnl = calculations::calculate_pnl(
                opening_price,
                trade::Price {
                    ask: execution_price,
                    bid: execution_price,
                },
                open_order.quantity,
                open_order.leverage,
                open_order.direction,
            )?;
            let amount_sats = trader_margin as i64 + pnl;

            (api::PaymentFlow::Inbound, amount_sats as u64)
        };

        let timestamp = order.creation_timestamp.unix_timestamp() as u64;

        let wallet_type = api::WalletType::Trade {
            order_id: order.id.to_string(),
        };

        trades.push(api::WalletHistoryItem {
            flow,
            amount_sats,
            timestamp,
            status: api::Status::Confirmed, // TODO: Support other order/trade statuses
            wallet_type,
        });
    }

    let history = chain![on_chain, off_chain, trades]
        .sorted_by(|a, b| b.timestamp.cmp(&a.timestamp))
        .collect();

    let wallet_info = api::WalletInfo {
        balances: wallet_balances.into(),
        history,
    };

    event::publish(&EventInternal::WalletInfoUpdateNotification(wallet_info));

    Ok(())
}

pub fn get_new_address() -> Result<String> {
    let address = NODE.get().inner.get_new_address()?;
    Ok(address.to_string())
}

/// TODO: remove this function once the lightning faucet is more stable. This is only added for
/// testing purposes - so that we can quickly get funds into the lightning wallet.
pub fn open_channel() -> Result<()> {
    NODE.get().inner.initiate_open_channel(
        config::get_coordinator_info().pubkey,
        500000,
        250000,
    )?;

    Ok(())
}

pub fn close_channel(is_force_close: bool) -> Result<()> {
    let node = NODE.try_get().context("failed to get ln dlc node")?;

    let channels = node.inner.list_channels();
    let channel_details = channels.first().context("No channel to close")?;

    node.inner
        .close_channel(channel_details.channel_id, is_force_close)?;

    Ok(())
}

pub fn create_invoice(amount_sats: Option<u64>) -> Result<Invoice> {
    let runtime = runtime()?;

    runtime.block_on(async {
        let node = NODE.get();
        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "http://{}/api/fake_scid/{}",
                config::get_http_endpoint(),
                node.inner.info.pubkey
            )) // TODO: make host configurable
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            bail!("Failed to fetch fake scid from coordinator: {text}")
        }

        let text = response.text().await?;
        tracing::info!("Fetch fake channel id: {}", text);

        let fake_channel_id: u64 = text.parse()?;

        node.inner.create_interceptable_invoice(
            amount_sats,
            fake_channel_id,
            config::get_coordinator_info().pubkey,
            0,
            "test".to_string(),
        )
    })
}

pub fn send_payment(invoice: &str) -> Result<()> {
    let invoice = Invoice::from_str(invoice).context("Could not parse Invoice string")?;
    NODE.get().inner.send_payment(&invoice)
}

pub async fn trade(trade_params: TradeParams) -> Result<(), (FailureReason, anyhow::Error)> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/api/trade", config::get_http_endpoint()))
        .json(&trade_params)
        .send()
        .await
        .context("Failed to register with coordinator")
        .map_err(|e| (FailureReason::TradeRequest, e))?;

    if !response.status().is_success() {
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                format!("could not decode response {err:#}")
            }
        };
        return Err((
            FailureReason::TradeResponse,
            anyhow!("Could not post trade to coordinator: {response_text}"),
        ));
    }

    tracing::info!("Sent trade request to coordinator successfully");

    Ok(())
}
