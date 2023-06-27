use crate::db;
use crate::db::trades::Trade;
use crate::position::models::NewPosition;
use crate::position::models::Position;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::secp256k1::PublicKey;
use coordinator_commons::TradeParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::contract::numerical_descriptor::NumericalDescriptor;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PayoutPoint;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use dlc_manager::ChannelId;
use dlc_messages::Message;
use lightning::ln::channelmanager::ChannelDetails;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::InMemoryStore;
use ln_dlc_node::FeeRateFallbacks;
use ln_dlc_node::WalletSettings;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use trade::cfd;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_short_liquidation_price;
use trade::cfd::BTCUSD_MAX_PRICE;
use trade::Direction;

pub mod connection;

/// The leverage used by the coordinator for all trades.
const COORDINATOR_LEVERAGE: f32 = 1.0;

#[derive(Debug, Clone)]
pub struct NodeSettings {
    // At times, we want to disallow opening new positions (e.g. before
    // scheduled upgrade)
    pub allow_opening_positions: bool,
    pub fallback_tx_fee_rate_normal: u32,
    pub fallback_tx_fee_rate_high_priority: u32,
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,
}

impl NodeSettings {
    fn as_wallet_settings(&self) -> WalletSettings {
        WalletSettings {
            max_allowed_tx_fee_rate_when_opening_channel: self
                .max_allowed_tx_fee_rate_when_opening_channel,
        }
    }

    fn as_fee_rate_fallbacks(&self) -> FeeRateFallbacks {
        FeeRateFallbacks {
            normal_priority: self.fallback_tx_fee_rate_normal,
            high_priority: self.fallback_tx_fee_rate_high_priority,
        }
    }
}

impl Default for NodeSettings {
    fn default() -> Self {
        Self {
            allow_opening_positions: true,
            fallback_tx_fee_rate_normal: 2000,
            fallback_tx_fee_rate_high_priority: 5000,
            max_allowed_tx_fee_rate_when_opening_channel: None,
        }
    }
}

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node<InMemoryStore>>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub settings: Arc<RwLock<NodeSettings>>,
}

impl Node {
    pub fn new(
        inner: Arc<ln_dlc_node::node::Node<InMemoryStore>>,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Self {
        Self {
            inner,
            pool,
            settings: Arc::new(RwLock::new(NodeSettings::default())),
        }
    }

    pub async fn update_settings(&self, settings: NodeSettings) {
        tracing::info!(?settings, "Updating node settings");
        *self.settings.write().await = settings.clone();

        // Forward relevant settings down to the wallet
        let wallet_settings = settings.as_wallet_settings();
        self.inner.wallet().update_settings(wallet_settings).await;

        let fee_rate_fallbacks = settings.as_fee_rate_fallbacks();
        self.inner
            .fee_rate_estimator
            .update_fallbacks(fee_rate_fallbacks);
    }

    /// Returns true or false, whether we can find an usable channel with the provided trader.
    ///
    /// Note, we use the usable channel to implicitely check if the user is connected, as it
    /// wouldn't be usable otherwise.
    pub fn is_connected(&self, trader: &PublicKey) -> bool {
        let usable_channels = self.inner.channel_manager.list_usable_channels();
        let usable_channels = usable_channels
            .iter()
            .filter(|channel| channel.is_usable && channel.counterparty.node_id == *trader)
            .collect::<Vec<_>>();

        if usable_channels.len() > 1 {
            tracing::warn!(peer_id=%trader, "Found more than one usable channel with trader");
        }
        !usable_channels.is_empty()
    }

    #[autometrics]
    pub async fn trade(&self, trade_params: &TradeParams) -> Result<()> {
        match self.decide_trade_action(&trade_params.pubkey)? {
            TradeAction::Open => {
                ensure!(
                    self.settings.read().await.allow_opening_positions,
                    "Opening positions is disabled"
                );
                self.open_position(trade_params).await?
            }
            TradeAction::Close(channel_id) => {
                let peer_id = trade_params.pubkey;
                tracing::info!(?trade_params, channel_id = %hex::encode(channel_id), %peer_id, "Closing position");

                let closing_price = trade_params.average_execution_price();

                let mut connection = self.pool.get()?;
                let position = match db::positions::Position::get_open_position_by_trader(
                    &mut connection,
                    trade_params.pubkey.to_string(),
                )? {
                    Some(position) => position,
                    None => bail!("Failed to find open position : {}", trade_params.pubkey),
                };

                self.close_position(&position, closing_price, channel_id)
                    .await?
            }
        };

        Ok(())
    }

    #[autometrics]
    async fn open_position(&self, trade_params: &TradeParams) -> Result<()> {
        let peer_id = trade_params.pubkey;
        tracing::info!(%peer_id, ?trade_params, "Opening position");

        let margin_trader = margin_trader(trade_params);
        let margin_coordinator = margin_coordinator(trade_params);

        let liquidation_price = liquidation_price(trade_params);

        let average_entry_price = trade_params
            .average_execution_price()
            .to_f32()
            .expect("to fit into f32");
        let new_position = NewPosition {
            contract_symbol: trade_params.contract_symbol,
            leverage: trade_params.leverage,
            quantity: trade_params.quantity,
            direction: trade_params.direction,
            trader: trade_params.pubkey,
            average_entry_price,
            liquidation_price,
            collateral: margin_coordinator as i64,
            expiry_timestamp: trade_params.filled_with.expiry_timestamp,
        };
        tracing::debug!(?new_position, "Inserting new position into db");

        let connection = &mut self.pool.get()?;
        let position = db::positions::Position::insert(connection, new_position.clone())?;
        db::trades::insert(
            connection,
            Trade {
                position_id: position.id,
                contract_symbol: new_position.contract_symbol.into(),
                trader_pubkey: new_position.trader.to_string(),
                quantity: new_position.quantity,
                leverage: new_position.leverage,
                collateral: new_position.collateral,
                direction: new_position.direction.into(),
                average_price: average_entry_price,
            },
        )?;

        let leverage_long = leverage_long(trade_params.direction, trade_params.leverage);
        let leverage_short = leverage_short(trade_params.direction, trade_params.leverage);

        let total_collateral = margin_coordinator + margin_trader;

        let contract_descriptor = build_contract_descriptor(
            total_collateral,
            trade_params.average_execution_price(),
            leverage_long,
            leverage_short,
        )
        .context("Could not build contract descriptor")?;

        let contract_symbol = trade_params.contract_symbol.label();
        let maturity_time = trade_params.filled_with.expiry_timestamp;
        let maturity_time = maturity_time.unix_timestamp();

        // The contract input to be used for setting up the trade between the trader and the
        // coordinator
        let event_id = format!("{contract_symbol}{maturity_time}");
        tracing::debug!(event_id, "Proposing dlc channel");
        let contract_input = ContractInput {
            offer_collateral: margin_coordinator,
            accept_collateral: margin_trader,
            fee_rate: 4,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![self.inner.oracle_pk()],
                    event_id,
                    threshold: 1,
                },
            }],
        };

        let channel_details = self.get_counterparty_channel(trade_params.pubkey)?;
        self.inner
            .propose_dlc_channel(channel_details, contract_input)
            .await
            .context("Could not propose dlc channel")?;
        Ok(())
    }

    #[autometrics]
    pub async fn close_position(
        &self,
        position: &Position,
        closing_price: Decimal,
        channel_id: ChannelId,
    ) -> Result<()> {
        let opening_price = Decimal::try_from(position.average_entry_price)?;

        let leverage_long = leverage_long(position.direction, position.leverage);
        let leverage_short = leverage_short(position.direction, position.leverage);

        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            position.quantity,
            leverage_long,
            leverage_short,
            position.direction,
        )?;

        tracing::debug!(
            ?position,
            channel_id = %hex::encode(channel_id),
            %accept_settlement_amount,
            "Closing position of {accept_settlement_amount} with {}",
            position.trader.to_string()
        );

        self.inner
            .propose_dlc_channel_collaborative_settlement(channel_id, accept_settlement_amount)
            .await?;

        let mut connection = self.pool.get()?;
        db::trades::insert(
            &mut connection,
            Trade {
                position_id: position.id,
                contract_symbol: position.contract_symbol.into(),
                trader_pubkey: position.trader.to_string(),
                quantity: position.quantity,
                leverage: position.leverage,
                collateral: position.collateral,
                direction: position.direction.opposite().into(),
                average_price: closing_price.to_f32().expect("To fit into f32"),
            },
        )?;

        db::positions::Position::set_open_position_to_closing(
            &mut connection,
            position.trader.to_string(),
        )
    }

    /// Decides what trade action should be performed according to the
    /// coordinator's current trading status with the trader.
    ///
    /// We look for a pre-existing position with the trader and
    /// instruct accordingly:
    ///
    /// 1. If a position of equal quantity and opposite direction is
    /// found, we direct the caller to close the position.
    ///
    /// 2. If no position is found, we direct the caller to open a
    /// position.
    ///
    /// 3. If a position of differing quantity is found, we direct the
    /// caller to extend or reduce the position. _This is currently
    /// not supported_.
    #[autometrics]
    pub fn decide_trade_action(&self, trader: &PublicKey) -> Result<TradeAction> {
        let action = match self.inner.get_dlc_channel_signed(trader)? {
            Some(subchannel) => {
                // FIXME: Should query the database for more
                // information

                // TODO: Detect if the position should be
                // extended/reduced. Return corresponding error as
                // this is currently not supported.

                TradeAction::Close(subchannel.channel_id)
            }
            None => TradeAction::Open,
        };

        Ok(action)
    }

    fn get_counterparty_channel(&self, trader_pubkey: PublicKey) -> Result<ChannelDetails> {
        let channel_details = self.inner.list_usable_channels();
        let channel_details = channel_details
            .into_iter()
            .find(|c| c.counterparty.node_id == trader_pubkey)
            .context("Channel details not found")?;
        Ok(channel_details)
    }

    #[autometrics]
    pub fn process_incoming_dlc_messages(&self) {
        let messages = self
            .inner
            .dlc_message_handler
            .get_and_clear_received_messages();

        for (node_id, msg) in messages {
            let msg_name = dlc_message_name(&msg);
            if let Err(e) = self.process_dlc_message(node_id, msg) {
                tracing::error!(
                    from = %node_id,
                    kind = %msg_name,
                    "Failed to process message: {e:#}"
                );
            }
        }
    }

    #[autometrics]
    fn process_dlc_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %dlc_message_name(&msg),
            "Processing message"
        );

        let resp = match msg {
            Message::OnChain(_) | Message::Channel(_) => self
                .inner
                .dlc_manager
                .on_dlc_message(&msg, node_id)
                .with_context(|| {
                    format!(
                        "Failed to handle {} message from {node_id}",
                        dlc_message_name(&msg)
                    )
                })?,
            Message::SubChannel(msg) => self
                .inner
                .sub_channel_manager
                .on_sub_channel_message(&msg, &node_id)
                .with_context(|| {
                    format!(
                        "Failed to handle {} message from {node_id}",
                        sub_channel_message_name(&msg)
                    )
                })?
                .map(Message::SubChannel),
        };

        if let Some(msg) = resp {
            tracing::info!(
                to = %node_id,
                kind = %dlc_message_name(&msg),
                "Sending message"
            );

            self.inner.dlc_message_handler.send_message(node_id, msg);
        }

        Ok(())
    }
}

pub enum TradeAction {
    Open,
    Close(ChannelId),
    // Extend,
    // Reduce,
}

/// Calculates the accept settlement amount based on the pnl.
fn calculate_accept_settlement_amount(
    opening_price: Decimal,
    closing_price: Decimal,
    quantity: f32,
    long_leverage: f32,
    short_leverage: f32,
    direction: Direction,
) -> Result<u64> {
    let pnl = cfd::calculate_pnl(
        opening_price,
        closing_price,
        quantity,
        long_leverage,
        short_leverage,
        direction,
    )?;

    let leverage = match direction {
        Direction::Long => long_leverage,
        Direction::Short => short_leverage,
    };

    let margin_trader = calculate_margin(opening_price, quantity, leverage);

    let accept_settlement_amount = Decimal::from(margin_trader) + Decimal::from(pnl);
    // the amount can only be positive, adding a safeguard here with the max comparison to
    // ensure the i64 fits into u64
    let accept_settlement_amount = accept_settlement_amount
        .max(Decimal::ZERO)
        .to_u64()
        .expect("to fit into u64");
    Ok(accept_settlement_amount)
}

fn margin_trader(trade_params: &TradeParams) -> u64 {
    calculate_margin(
        trade_params.average_execution_price(),
        trade_params.quantity,
        trade_params.leverage,
    )
}

fn margin_coordinator(trade_params: &TradeParams) -> u64 {
    calculate_margin(
        trade_params.average_execution_price(),
        trade_params.quantity,
        COORDINATOR_LEVERAGE,
    )
}

fn liquidation_price(trade_params: &TradeParams) -> f32 {
    let price = trade_params.average_execution_price();
    let leverage = Decimal::try_from(trade_params.leverage).expect("to fit into decimal");

    match trade_params.direction {
        Direction::Long => calculate_long_liquidation_price(leverage, price),
        Direction::Short => calculate_short_liquidation_price(leverage, price),
    }
    .to_f32()
    .expect("to fit into f32")
}

fn leverage_long(direction: Direction, trader_leverage: f32) -> f32 {
    match direction {
        Direction::Long => trader_leverage,
        Direction::Short => COORDINATOR_LEVERAGE,
    }
}

fn leverage_short(direction: Direction, trader_leverage: f32) -> f32 {
    match direction {
        Direction::Long => COORDINATOR_LEVERAGE,
        Direction::Short => trader_leverage,
    }
}

fn get_rounding_intervals() -> RoundingIntervals {
    RoundingIntervals {
        intervals: vec![RoundingInterval {
            begin_interval: 0,
            rounding_mod: 500,
        }],
    }
}

/// Builds the contract descriptor from the point of view of the trader.
fn build_contract_descriptor(
    total_collateral: u64,
    initial_price: Decimal,
    leverage_long: f32,
    leverage_short: f32,
) -> Result<ContractDescriptor> {
    Ok(ContractDescriptor::Numerical(NumericalDescriptor {
        payout_function: build_payout_function(
            total_collateral,
            initial_price,
            leverage_long,
            leverage_short,
        )?,
        rounding_intervals: get_rounding_intervals(),
        difference_params: None,
        oracle_numeric_infos: dlc_trie::OracleNumericInfo {
            base: 2,
            nb_digits: vec![20],
        },
    }))
}

/// Builds a [`PayoutFunction`].
///
/// TODO: We are currently building a linear payout function for
/// simplicity. This is *wrong*. We should build an inverse payout
/// function like we used to do in ItchySats.
fn build_payout_function(
    total_collateral: u64,
    initial_price: Decimal,
    leverage_long: f32,
    leverage_short: f32,
) -> Result<PayoutFunction> {
    let leverage_short = Decimal::try_from(leverage_short)?;
    let liquidation_price_short = calculate_short_liquidation_price(leverage_short, initial_price);

    let leverage_long = Decimal::try_from(leverage_long)?;
    let liquidation_price_long = calculate_long_liquidation_price(leverage_long, initial_price);

    let lower_limit = liquidation_price_long
        .floor()
        .to_u64()
        .expect("Failed to fit floored liquidation price to u64");
    let upper_limit = liquidation_price_short
        .floor()
        .to_u64()
        .expect("Failed to fit floored liquidation price to u64");

    let lower_range = PolynomialPayoutCurvePiece::new(vec![
        PayoutPoint {
            event_outcome: 0,
            outcome_payout: 0,
            extra_precision: 0,
        },
        PayoutPoint {
            event_outcome: lower_limit,
            outcome_payout: 0,
            extra_precision: 0,
        },
    ])?;

    let middle_range = PolynomialPayoutCurvePiece::new(vec![
        PayoutPoint {
            event_outcome: lower_limit,
            outcome_payout: 0,
            extra_precision: 0,
        },
        PayoutPoint {
            event_outcome: upper_limit,
            outcome_payout: total_collateral,
            extra_precision: 0,
        },
    ])?;

    let mut pieces = vec![
        PayoutFunctionPiece::PolynomialPayoutCurvePiece(lower_range),
        PayoutFunctionPiece::PolynomialPayoutCurvePiece(middle_range),
    ];

    // When the upper limit is greater than or equal to the
    // `BTCUSD_MAX_PRICE`, we don't have to add another curve piece.
    if upper_limit < BTCUSD_MAX_PRICE {
        let upper_range = PolynomialPayoutCurvePiece::new(vec![
            PayoutPoint {
                event_outcome: upper_limit,
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
            PayoutPoint {
                event_outcome: BTCUSD_MAX_PRICE,
                outcome_payout: total_collateral,
                extra_precision: 0,
            },
        ])?;

        pieces.push(PayoutFunctionPiece::PolynomialPayoutCurvePiece(upper_range));
    }

    let payout_function = PayoutFunction::new(pieces)?;

    Ok(payout_function)
}

#[cfg(test)]
pub mod tests {
    use crate::node::calculate_accept_settlement_amount;
    use rust_decimal::Decimal;
    use trade::cfd::calculate_margin;
    use trade::Direction;

    // some basic sanity tests, that in case the position goes the right or wrong way the settlement
    // amount is moving correspondingly up or down.

    #[test]
    fn given_a_long_position_and_a_larger_closing_price() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount > margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_larger_closing_price() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_long_position_and_a_smaller_closing_price() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_smaller_closing_price() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            1.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount > margin_trader);
    }

    #[test]
    fn given_a_long_position_and_a_larger_closing_price_different_leverages() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            2.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 2.0);
        assert!(accept_settlement_amount > margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_larger_closing_price_different_leverages() {
        let opening_price = Decimal::from(22000);
        let closing_price = Decimal::from(23000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            2.0,
            1.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 1.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_long_position_and_a_smaller_closing_price_different_leverages() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            2.0,
            1.0,
            Direction::Long,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 2.0);
        assert!(accept_settlement_amount < margin_trader);
    }

    #[test]
    fn given_a_short_position_and_a_smaller_closing_price_different_leverages() {
        let opening_price = Decimal::from(23000);
        let closing_price = Decimal::from(22000);
        let quantity: f32 = 1.0;
        let accept_settlement_amount = calculate_accept_settlement_amount(
            opening_price,
            closing_price,
            quantity,
            1.0,
            2.0,
            Direction::Short,
        )
        .unwrap();

        let margin_trader = calculate_margin(opening_price, quantity, 2.0);
        assert!(accept_settlement_amount > margin_trader);
    }
}
