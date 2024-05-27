use crate::calculations;
use crate::channel_trade_constraints;
use crate::channel_trade_constraints::TradeConstraints;
use crate::commons::api::Price;
use crate::config;
use crate::config::api::Config;
use crate::config::api::Directories;
use crate::config::get_network;
use crate::db;
use crate::destination;
use crate::dlc;
use crate::dlc::get_storage;
pub use crate::dlc_channel::ChannelState;
pub use crate::dlc_channel::DlcChannel;
pub use crate::dlc_channel::SignedChannelState;
use crate::emergency_kit;
use crate::event;
use crate::event::api::FlutterSubscriber;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::health;
use crate::logger;
use crate::max_quantity::max_quantity;
use crate::polls;
use crate::trade::order;
use crate::trade::order::api::NewOrder;
use crate::trade::order::api::Order;
use crate::trade::position;
use crate::trade::position::api::Position;
use crate::trade::users;
use crate::unfunded_channel_opening_order;
use crate::unfunded_channel_opening_order::ExternalFunding;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bdk::FeeRate;
use bitcoin::Amount;
use flutter_rust_bridge::StreamSink;
use flutter_rust_bridge::SyncReturn;
use lightning::chain::chaininterface::ConfirmationTarget as LnConfirmationTarget;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::backtrace::Backtrace;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::channel;
use xxi_node::commons::ChannelOpeningParams;
pub use xxi_node::commons::ContractSymbol;
pub use xxi_node::commons::Direction;
use xxi_node::commons::OrderbookRequest;
use xxi_node::seed::Bip39Seed;

/// Initialise logging infrastructure for Rust
pub fn init_logging(sink: StreamSink<logger::LogEntry>) {
    logger::create_log_stream(sink)
}

#[derive(Clone, Debug, Default)]
pub struct TenTenOneConfig {
    pub liquidity_options: Vec<LiquidityOption>,
    pub min_quantity: u64,
    pub maintenance_margin_rate: f32,
    pub referral_status: ReferralStatus,
}

impl From<xxi_node::commons::TenTenOneConfig> for TenTenOneConfig {
    fn from(value: xxi_node::commons::TenTenOneConfig) -> Self {
        Self {
            liquidity_options: value
                .liquidity_options
                .into_iter()
                .map(|lo| lo.into())
                .collect(),
            min_quantity: value.min_quantity,
            maintenance_margin_rate: value.maintenance_margin_rate,
            referral_status: value.referral_status.into(),
        }
    }
}

/// Assembles the wallet info and publishes wallet info update event.
#[tokio::main(flavor = "current_thread")]
pub async fn refresh_wallet_info() -> Result<()> {
    dlc::refresh_wallet_info().await?;

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn full_sync(stop_gap: usize) -> Result<()> {
    dlc::full_sync(stop_gap).await?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct Poll {
    pub id: i32,
    pub poll_type: PollType,
    pub question: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Clone)]
pub struct Choice {
    pub id: i32,
    pub value: String,
    pub editable: bool,
}

#[derive(Debug, Clone)]
pub enum PollType {
    SingleChoice,
}

impl From<xxi_node::commons::Poll> for Poll {
    fn from(value: xxi_node::commons::Poll) -> Self {
        Poll {
            id: value.id,
            poll_type: value.poll_type.into(),
            question: value.question,
            choices: value
                .choices
                .into_iter()
                .map(|choice| choice.into())
                .collect(),
        }
    }
}

impl From<xxi_node::commons::PollType> for PollType {
    fn from(value: xxi_node::commons::PollType) -> Self {
        match value {
            xxi_node::commons::PollType::SingleChoice => PollType::SingleChoice,
        }
    }
}

impl From<xxi_node::commons::Choice> for Choice {
    fn from(value: xxi_node::commons::Choice) -> Self {
        Choice {
            id: value.id,
            value: value.value,
            editable: value.editable,
        }
    }
}

impl From<Choice> for xxi_node::commons::Choice {
    fn from(value: Choice) -> Self {
        xxi_node::commons::Choice {
            id: value.id,
            value: value.value,
            editable: value.editable,
        }
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn fetch_poll() -> Result<Option<Poll>> {
    let polls: Vec<Poll> = polls::get_new_polls()
        .await?
        .into_iter()
        .map(|poll| poll.into())
        .collect();
    // For now we just return the first poll
    Ok(polls.first().cloned())
}

#[tokio::main(flavor = "current_thread")]
pub async fn post_selected_choice(selected_choice: Choice, poll_id: i32) -> Result<()> {
    let trader_pk = dlc::get_node_pubkey();
    polls::answer_poll(selected_choice.into(), poll_id, trader_pk).await?;
    Ok(())
}

pub fn reset_all_answered_polls() -> Result<SyncReturn<()>> {
    db::delete_answered_poll_cache()?;
    Ok(SyncReturn(()))
}

pub fn ignore_poll(poll_id: i32) -> Result<SyncReturn<()>> {
    polls::ignore_poll(poll_id)?;
    Ok(SyncReturn(()))
}

#[derive(Clone, Debug)]
pub struct WalletHistoryItem {
    pub flow: PaymentFlow,
    pub amount_sats: u64,
    pub timestamp: u64,
    pub status: Status,
    pub wallet_type: WalletHistoryItemType,
}

#[derive(Clone, Debug)]
pub enum WalletHistoryItemType {
    OnChain {
        txid: String,
        fee_sats: Option<u64>,
        confirmations: u64,
    },
    Lightning {
        payment_hash: String,
        description: String,
        payment_preimage: Option<String>,
        invoice: Option<String>,
        fee_msat: Option<u64>,
        expiry_timestamp: Option<u64>,
        funding_txid: Option<String>,
    },
    Trade {
        order_id: String,
        fee_sat: u64,
        pnl: Option<i64>,
        contracts: u64,
        direction: String,
    },
    DlcChannelFunding {
        funding_txid: String,
        // This fee represents the total fee reserved for all off-chain transactions, i.e. for the
        // fund/buffer/cet/refund. Only the part for the fund tx has been paid for now
        funding_tx_fee_sats: Option<u64>,
        confirmations: u64,
        // The amount we hold in the channel
        our_channel_input_amount_sats: u64,
    },
}

#[derive(Clone, Debug, Default, Copy)]
pub enum PaymentFlow {
    #[default]
    Inbound,
    Outbound,
}

impl fmt::Display for PaymentFlow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PaymentFlow::Inbound => write!(f, "inbound"),
            PaymentFlow::Outbound => write!(f, "outbound"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Pending,
    Confirmed,
    Expired,
    Failed,
}

pub fn calculate_margin(price: f32, quantity: f32, leverage: f32) -> SyncReturn<u64> {
    SyncReturn(calculations::calculate_margin(price, quantity, leverage))
}

pub fn calculate_quantity(price: f32, margin: u64, leverage: f32) -> SyncReturn<f32> {
    SyncReturn(calculations::calculate_quantity(price, margin, leverage))
}

pub fn calculate_liquidation_price(
    price: f32,
    leverage: f32,
    direction: Direction,
) -> SyncReturn<f32> {
    let maintenance_margin_rate = dlc::get_maintenance_margin_rate();
    SyncReturn(calculations::calculate_liquidation_price(
        price,
        leverage,
        direction,
        maintenance_margin_rate,
    ))
}

pub fn calculate_pnl(
    opening_price: f32,
    closing_price: Price,
    quantity: f32,
    leverage: f32,
    direction: Direction,
) -> SyncReturn<i64> {
    // TODO: Handle the result and don't just return 0

    SyncReturn(
        calculations::calculate_pnl(
            opening_price,
            closing_price.into(),
            quantity,
            leverage,
            direction,
        )
        .unwrap_or(0),
    )
}

/// Calculate the order matching fee that the app user will have to pay for if the corresponding
/// trade gets executed.
///
/// This is only an estimate as the price may change slightly. Also, the coordinator could choose to
/// change the fee structure independently.
pub fn order_matching_fee(quantity: f32, price: f32) -> SyncReturn<u64> {
    let price = Decimal::from_f32(price).expect("price to fit in Decimal");

    let fee_rate = dlc::get_order_matching_fee_rate(false);
    let order_matching_fee =
        xxi_node::commons::order_matching_fee(quantity, price, fee_rate).to_sat();

    SyncReturn(order_matching_fee)
}

/// Calculates the max quantity the user is able to trade considering the trader and the coordinator
/// balances and constraints. Note, this is not an exact maximum, but a very close approximation.
pub fn calculate_max_quantity(
    price: f32,
    trader_leverage: f32,
    trader_direction: Direction,
) -> SyncReturn<u64> {
    let price = Decimal::from_f32(price).expect("to fit");

    let max_quantity =
        max_quantity(price, trader_leverage, trader_direction).unwrap_or(Decimal::ZERO);
    let max_quantity = max_quantity.floor().to_u64().expect("to fit");

    SyncReturn(max_quantity)
}

#[tokio::main(flavor = "current_thread")]
pub async fn submit_order(order: NewOrder) -> Result<String> {
    order::handler::submit_order(order.into(), None)
        .await
        .map_err(anyhow::Error::new)
        .map(|id| id.to_string())
}

#[tokio::main(flavor = "current_thread")]
pub async fn submit_channel_opening_order(
    order: NewOrder,
    coordinator_reserve: u64,
    trader_reserve: u64,
) -> Result<String> {
    order::handler::submit_order(
        order.into(),
        Some(ChannelOpeningParams {
            coordinator_reserve: Amount::from_sat(coordinator_reserve),
            trader_reserve: Amount::from_sat(trader_reserve),
            pre_image: None,
        }),
    )
    .await
    .map_err(anyhow::Error::new)
    .map(|id| id.to_string())
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_orders() -> Result<Vec<Order>> {
    let orders = order::handler::get_orders_for_ui()
        .await?
        .into_iter()
        .map(|order| order.into())
        .collect::<Vec<Order>>();

    Ok(orders)
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_positions() -> Result<Vec<Position>> {
    let positions = position::handler::get_positions()?
        .into_iter()
        .map(|order| order.into())
        .collect::<Vec<Position>>();

    Ok(positions)
}

pub fn set_filling_orders_to_failed() -> Result<()> {
    emergency_kit::set_filling_orders_to_failed()
}

pub fn delete_position() -> Result<()> {
    emergency_kit::delete_position()
}

pub fn recreate_position() -> Result<()> {
    emergency_kit::recreate_position()
}

pub fn resend_settle_finalize_message() -> Result<()> {
    emergency_kit::resend_settle_finalize_message()
}

pub fn subscribe(stream: StreamSink<event::api::Event>) {
    tracing::debug!("Subscribing flutter to event hub");
    event::subscribe(FlutterSubscriber::new(stream))
}

/// Wrapper for Flutter purposes - can throw an exception.
pub fn run_in_flutter(seed_dir: String, fcm_token: String) -> Result<()> {
    match crate::state::try_get_websocket() {
        None => {
            let (tx_websocket, _rx) = channel::<OrderbookRequest>(10);
            run_internal(
                seed_dir,
                fcm_token,
                tx_websocket.clone(),
                IncludeBacktraceOnPanic::Yes,
            )
            .context("Failed to start the backend")?;

            crate::state::set_websocket(tx_websocket);
        }
        Some(tx_websocket) => {
            // In case of a hot-restart we do not start the node again as it is already running.
            // However, we need to re-send the authentication message to get the initial data from
            // the coordinator and trigger a new user login event.
            tracing::info!("Re-sending authentication message");

            let signature = orderbook_client::create_auth_message_signature(move |msg| {
                xxi_node::commons::Signature {
                    pubkey: dlc::get_node_pubkey(),
                    signature: dlc::get_node_key().sign_ecdsa(msg),
                }
            });

            let version = env!("CARGO_PKG_VERSION").to_string();
            let os = std::env::consts::OS.to_string();
            let runtime = crate::state::get_or_create_tokio_runtime()?;
            runtime.block_on(async {
                tx_websocket.send(OrderbookRequest::Authenticate {
                    fcm_token: Some(fcm_token),
                    version: Some(version),
                    os: Some(os),
                    signature,
                })
            })?;
        }
    };

    Ok(())
}

/// Wrapper for the tests.
///
/// Needed as we do not want to have a hot restart handling in the tests and also can't expose a
/// channel::Sender through frb.
pub fn run_in_test(seed_dir: String) -> Result<()> {
    let (tx_websocket, _rx) = channel::<OrderbookRequest>(10);
    run_internal(
        seed_dir,
        "".to_string(),
        tx_websocket,
        IncludeBacktraceOnPanic::No,
    )
}

#[derive(PartialEq)]
pub enum IncludeBacktraceOnPanic {
    Yes,
    No,
}

pub fn set_config(config: Config, app_dir: String, seed_dir: String) -> Result<()> {
    crate::state::set_config((config, Directories { app_dir, seed_dir }).into());
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn full_backup() -> Result<()> {
    db::init_db(&config::get_data_dir(), get_network())?;
    get_storage().full_backup().await
}

fn run_internal(
    seed_dir: String,
    fcm_token: String,
    tx_websocket: broadcast::Sender<OrderbookRequest>,
    backtrace_on_panic: IncludeBacktraceOnPanic,
) -> Result<()> {
    if backtrace_on_panic == IncludeBacktraceOnPanic::Yes {
        std::panic::set_hook(
            #[allow(clippy::print_stderr)]
            Box::new(|info| {
                let backtrace = Backtrace::force_capture();

                tracing::error!(%info, "Aborting after panic in task");
                eprintln!("{backtrace}");

                std::process::abort()
            }),
        );
    }

    db::init_db(&config::get_data_dir(), get_network())?;

    let runtime = crate::state::get_or_create_tokio_runtime()?;

    let seed_dir = Path::new(&seed_dir).join(get_network().to_string());
    let seed_path = seed_dir.join("seed");
    let seed = Bip39Seed::initialize(&seed_path)?;

    crate::state::set_seed(seed.clone());

    let (_health, tx) = health::Health::new(runtime);

    dlc::run(runtime, tx, fcm_token, tx_websocket)
}

pub fn get_new_address() -> Result<String> {
    dlc::get_new_address()
}

pub fn get_unused_address() -> Result<String> {
    dlc::get_unused_address()
}

#[tokio::main(flavor = "current_thread")]
pub async fn close_channel() -> Result<()> {
    event::publish(&EventInternal::BackgroundNotification(
        BackgroundTask::CloseChannel(TaskStatus::Pending),
    ));

    let fail = |e: &anyhow::Error| {
        event::publish(&EventInternal::BackgroundNotification(
            BackgroundTask::CloseChannel(TaskStatus::Failed(format!("{e:#}"))),
        ))
    };

    dlc::close_channel(false).await.inspect_err(fail)?;
    // wait a bit so that the sync can find the the broadcasted transaction.
    tokio::time::sleep(Duration::from_millis(500)).await;
    dlc::refresh_wallet_info().await.inspect_err(fail)?;

    event::publish(&EventInternal::BackgroundNotification(
        BackgroundTask::CloseChannel(TaskStatus::Success),
    ));

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn force_close_channel() -> Result<()> {
    dlc::close_channel(true).await
}

pub fn channel_trade_constraints() -> Result<SyncReturn<TradeConstraints>> {
    let trade_constraints = channel_trade_constraints::channel_trade_constraints()?;
    Ok(SyncReturn(trade_constraints))
}

#[derive(Debug, Clone)]
pub struct LiquidityOption {
    pub id: i32,
    pub rank: usize,
    pub title: String,
    /// the amount the trader can trade up to
    pub trade_up_to_sats: u64,
    /// min deposit in sats
    pub min_deposit_sats: u64,
    /// max deposit in sats
    pub max_deposit_sats: u64,
    /// min fee in sats
    pub min_fee_sats: u64,
    pub fee_percentage: f64,
    pub coordinator_leverage: f32,
    pub active: bool,
}

impl From<xxi_node::commons::LiquidityOption> for LiquidityOption {
    fn from(value: xxi_node::commons::LiquidityOption) -> Self {
        LiquidityOption {
            id: value.id,
            rank: value.rank,
            title: value.title,
            trade_up_to_sats: value.trade_up_to_sats,
            min_deposit_sats: value.min_deposit_sats,
            max_deposit_sats: value.max_deposit_sats,
            min_fee_sats: value.min_fee_sats,
            fee_percentage: value.fee_percentage,
            coordinator_leverage: value.coordinator_leverage,
            active: value.active,
        }
    }
}

pub struct PaymentRequest {
    pub address: String,
    pub bip21: String,
}

pub fn create_payment_request(
    amount_sats: Option<u64>,
    _description: String,
) -> Result<PaymentRequest> {
    let amount_query = amount_sats
        .map(|amt| format!("?amount={}", Amount::from_sat(amt).to_btc()))
        .unwrap_or_default();
    let addr = dlc::get_unused_address()?;

    Ok(PaymentRequest {
        bip21: format!("bitcoin:{addr}{amount_query}"),
        address: addr,
    })
}

/// Fee configuration for an on-chaint transaction.
pub enum FeeConfig {
    /// The fee rate is derived from the configured priority.
    Priority(ConfirmationTarget),
    /// The fee rate is explicitly configured.
    FeeRate { sats_per_vbyte: f32 },
}

impl From<FeeConfig> for xxi_node::FeeConfig {
    fn from(value: FeeConfig) -> Self {
        match value {
            FeeConfig::Priority(target) => xxi_node::FeeConfig::Priority(target.into()),
            FeeConfig::FeeRate { sats_per_vbyte } => {
                xxi_node::FeeConfig::FeeRate(FeeRate::from_sat_per_vb(sats_per_vbyte))
            }
        }
    }
}

/// Analogous to [`lightning::chain::chaininterface::ConfirmationTarget`] but for the Flutter API
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ConfirmationTarget {
    Minimum,
    Background,
    Normal,
    HighPriority,
}

impl From<ConfirmationTarget> for LnConfirmationTarget {
    fn from(value: ConfirmationTarget) -> Self {
        match value {
            ConfirmationTarget::Minimum => LnConfirmationTarget::MempoolMinimum,
            ConfirmationTarget::Background => LnConfirmationTarget::Background,
            ConfirmationTarget::Normal => LnConfirmationTarget::Normal,
            ConfirmationTarget::HighPriority => LnConfirmationTarget::HighPriority,
        }
    }
}

pub struct FeeEstimation {
    pub sats_per_vbyte: f32,
    pub total_sats: u64,
}

/// Calculate the fees for an on-chain transaction, using the 3 default fee rates (background,
/// normal, and high priority). This both estimates the fee rate and calculates the TX size to get
/// the overall fee for a given TX.
pub fn calculate_all_fees_for_on_chain(address: String) -> Result<Vec<FeeEstimation>> {
    const TARGETS: [ConfirmationTarget; 4] = [
        ConfirmationTarget::Minimum,
        ConfirmationTarget::Background,
        ConfirmationTarget::Normal,
        ConfirmationTarget::HighPriority,
    ];

    let runtime = crate::state::get_or_create_tokio_runtime()?;
    runtime.block_on(async {
        let mut fees = Vec::with_capacity(TARGETS.len());

        for confirmation_target in TARGETS {
            let fee_rate_sats_per_vb = fee_rate(confirmation_target);

            let fee_config = FeeConfig::Priority(confirmation_target);
            let absolute_fee = dlc::estimate_payment_fee(&address, fee_config).await?;

            fees.push(FeeEstimation {
                sats_per_vbyte: fee_rate_sats_per_vb,
                total_sats: absolute_fee.to_sat(),
            })
        }

        Ok(fees)
    })
}

#[tokio::main(flavor = "current_thread")]
pub async fn calculate_fee_estimate(
    address: String,
    fee_rate_sats_per_vb: f32,
) -> Result<FeeEstimation> {
    let estimate = dlc::estimate_payment_fee(
        &address,
        FeeConfig::FeeRate {
            sats_per_vbyte: fee_rate_sats_per_vb,
        },
    )
    .await?;

    Ok(FeeEstimation {
        sats_per_vbyte: fee_rate_sats_per_vb,
        total_sats: estimate.to_sat(),
    })
}

pub fn fee_rate(confirmation_target: ConfirmationTarget) -> f32 {
    dlc::get_fee_rate_for_target(confirmation_target.into()).as_sat_per_vb()
}

#[tokio::main(flavor = "current_thread")]
pub async fn send_payment(amount: u64, address: String, fee: FeeConfig) -> Result<String> {
    let txid = dlc::send_payment(amount, address, fee).await?;

    Ok(txid.to_string())
}

pub struct LastLogin {
    pub id: i32,
    pub date: String,
}

pub fn get_seed_phrase() -> SyncReturn<Vec<String>> {
    SyncReturn(dlc::get_seed_phrase())
}

#[tokio::main(flavor = "current_thread")]
pub async fn restore_from_seed_phrase(
    seed_phrase: String,
    target_seed_file_path: String,
) -> Result<()> {
    let file_path = PathBuf::from(target_seed_file_path);
    tracing::info!("Restoring seed from phrase to {:?}", file_path);
    dlc::restore_from_mnemonic(&seed_phrase, file_path.as_path()).await?;
    Ok(())
}

pub fn init_new_mnemonic(target_seed_file_path: String) -> Result<()> {
    let file_path = PathBuf::from(target_seed_file_path);
    tracing::info!("Creating a new seed in {:?}", file_path);
    dlc::init_new_mnemonic(file_path.as_path())
}

/// Enroll or update a user in the beta program
#[tokio::main(flavor = "current_thread")]
pub async fn register_beta(contact: String, referral_code: Option<String>) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION").to_string();

    users::register_beta(contact, version, referral_code).await
}

#[derive(Debug)]
pub struct User {
    pub pubkey: String,
    pub contact: Option<String>,
    pub nickname: Option<String>,
}

impl From<xxi_node::commons::User> for User {
    fn from(value: xxi_node::commons::User) -> Self {
        User {
            pubkey: value.pubkey.to_string(),
            contact: value.contact,
            nickname: value.nickname,
        }
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_user_details() -> Result<User> {
    users::get_user_details().await.map(|user| user.into())
}

pub enum Destination {
    Bolt11 {
        description: String,
        amount_sats: u64,
        timestamp: u64,
        payee: String,
        expiry: u64,
    },
    OnChainAddress(String),
    Bip21 {
        address: String,
        label: String,
        message: String,
        amount_sats: Option<u64>,
    },
}

pub fn decode_destination(destination: String) -> Result<Destination> {
    ensure!(!destination.is_empty(), "Destination must be set");
    destination::decode_destination(destination)
}

pub fn get_node_id() -> SyncReturn<String> {
    SyncReturn(dlc::get_node_pubkey().to_string())
}

pub fn get_estimated_channel_fee_reserve() -> Result<SyncReturn<u64>> {
    let reserve = dlc::estimated_fee_reserve()?;

    Ok(SyncReturn(reserve.to_sat()))
}

pub fn get_estimated_funding_tx_fee() -> Result<SyncReturn<u64>> {
    let fee = dlc::estimated_funding_tx_fee()?;

    Ok(SyncReturn(fee.to_sat()))
}

pub fn get_expiry_timestamp(network: String) -> SyncReturn<i64> {
    let network = config::api::parse_network(&network);
    SyncReturn(
        xxi_node::commons::calculate_next_expiry(OffsetDateTime::now_utc(), network)
            .unix_timestamp(),
    )
}

pub fn get_dlc_channel_id() -> Result<Option<String>> {
    let dlc_channel_id =
        dlc::get_signed_dlc_channel()?.map(|channel| hex::encode(channel.channel_id));

    Ok(dlc_channel_id)
}

pub fn list_dlc_channels() -> Result<Vec<DlcChannel>> {
    let channels = dlc::list_dlc_channels()?
        .iter()
        .map(dlc::DlcChannel::from)
        .map(DlcChannel::from)
        .collect();
    Ok(channels)
}

pub fn delete_dlc_channel(dlc_channel_id: String) -> Result<()> {
    emergency_kit::delete_dlc_channel(dlc_channel_id)
}

pub fn get_new_random_name() -> SyncReturn<String> {
    SyncReturn(crate::names::get_new_name())
}

#[tokio::main(flavor = "current_thread")]
pub async fn update_nickname(nickname: String) -> Result<()> {
    users::update_username(nickname).await
}

pub fn roll_back_channel_state() -> Result<()> {
    tracing::warn!(
        "Executing emergency kit! Attempting to rollback channel state to last stable state"
    );
    dlc::roll_back_channel_state()
}

#[derive(Clone, Debug, Default)]
pub struct ReferralStatus {
    pub referral_code: String,
    pub number_of_activated_referrals: usize,
    pub number_of_total_referrals: usize,
    pub referral_tier: usize,
    pub referral_fee_bonus: f32,
    /// The type of this referral status
    pub bonus_status_type: BonusStatusType,
}

#[derive(Clone, Debug, Default)]
pub enum BonusStatusType {
    #[default]
    None,
    /// The bonus is because he referred enough users
    Referral,
    /// The user has been referred and gets a bonus
    Referent,
}

impl From<xxi_node::commons::BonusStatusType> for BonusStatusType {
    fn from(value: xxi_node::commons::BonusStatusType) -> Self {
        match value {
            xxi_node::commons::BonusStatusType::Referral => BonusStatusType::Referral,
            xxi_node::commons::BonusStatusType::Referent => BonusStatusType::Referent,
        }
    }
}

impl From<xxi_node::commons::ReferralStatus> for ReferralStatus {
    fn from(value: xxi_node::commons::ReferralStatus) -> Self {
        ReferralStatus {
            referral_code: value.referral_code,
            referral_tier: value.referral_tier,
            number_of_activated_referrals: value.number_of_activated_referrals,
            number_of_total_referrals: value.number_of_total_referrals,
            referral_fee_bonus: value.referral_fee_bonus.to_f32().expect("to fit into f32"),
            bonus_status_type: value
                .bonus_status_type
                .map(|status| status.into())
                .unwrap_or_default(),
        }
    }
}

/// Returns true if the user has at least a single trade in his db
pub fn has_traded_once() -> Result<SyncReturn<bool>> {
    Ok(SyncReturn(!db::get_all_trades()?.is_empty()))
}

#[tokio::main(flavor = "current_thread")]
pub async fn submit_unfunded_channel_opening_order(
    order: NewOrder,
    coordinator_reserve: u64,
    trader_reserve: u64,
    estimated_margin: u64,
) -> Result<ExternalFunding> {
    unfunded_channel_opening_order::submit_unfunded_channel_opening_order(
        order,
        coordinator_reserve,
        trader_reserve,
        estimated_margin,
    )
    .await
}

#[tokio::main(flavor = "current_thread")]
pub async fn abort_unfunded_channel_opening_order() -> Result<()> {
    unfunded_channel_opening_order::abort_watcher().await
}
