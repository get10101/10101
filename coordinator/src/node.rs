use crate::db;
use crate::node::storage::NodeStorage;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use crate::payout_curve;
use crate::payout_curve::create_rounding_interval;
use crate::position::models::NewPosition;
use crate::position::models::Position;
use crate::position::models::PositionState;
use crate::storage::CoordinatorTenTenOneStorage;
use crate::trade::models::NewTrade;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::secp256k1::PublicKey;
use coordinator_commons::TradeParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::Connection;
use diesel::PgConnection;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::ChannelId;
use dlc_manager::ContractId;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::util::config::UserConfig;
use ln_dlc_node::node;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::send_dlc_message;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::RunningNode;
use ln_dlc_node::WalletSettings;
use orderbook_commons::order_matching_fee_taker;
use orderbook_commons::MatchState;
use orderbook_commons::OrderState;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;
use uuid::Uuid;

pub mod closed_positions;
pub mod connection;
pub mod expired_positions;
pub mod rollover;
pub mod routing_fees;
pub mod storage;
pub mod unrealized_pnl;

#[derive(Debug, Clone)]
pub struct NodeSettings {
    // At times, we want to disallow opening new positions (e.g. before
    // scheduled upgrade)
    pub allow_opening_positions: bool,
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,
    /// Defines the sats/vbyte to be used for all transactions within the sub-channel
    pub contract_tx_fee_rate: u64,
}

impl NodeSettings {
    fn to_wallet_settings(&self) -> WalletSettings {
        WalletSettings {
            max_allowed_tx_fee_rate_when_opening_channel: self
                .max_allowed_tx_fee_rate_when_opening_channel,
        }
    }
}

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<node::Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    _running: Arc<RunningNode>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    settings: Arc<RwLock<NodeSettings>>,
}

impl Node {
    pub fn new(
        inner: Arc<node::Node<CoordinatorTenTenOneStorage, NodeStorage>>,
        running: RunningNode,
        pool: Pool<ConnectionManager<PgConnection>>,
        settings: NodeSettings,
    ) -> Self {
        Self {
            inner,
            pool,
            settings: Arc::new(RwLock::new(settings)),
            _running: Arc::new(running),
        }
    }

    pub async fn update_settings(&self, settings: NodeSettings) {
        tracing::info!(?settings, "Updating node settings");
        *self.settings.write().await = settings.clone();

        // Forward relevant settings down to the wallet
        let wallet_settings = settings.to_wallet_settings();
        self.inner.wallet().update_settings(wallet_settings).await;
    }

    pub fn update_ldk_settings(&self, ldk_config: UserConfig) {
        self.inner.update_ldk_settings(ldk_config)
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

    pub async fn trade(&self, trade_params: &TradeParams) -> Result<()> {
        let mut connection = self.pool.get()?;
        let order_id = trade_params.filled_with.order_id;
        let trader_id = trade_params.pubkey;
        match self.trade_internal(trade_params, &mut connection).await {
            Ok(()) => {
                tracing::info!(
                    %trader_id,
                    %order_id,
                    "Successfully processed match, setting match to Filled"
                );

                update_order_and_match(
                    &mut connection,
                    order_id,
                    MatchState::Filled,
                    OrderState::Taken,
                )?;
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    %trader_id,
                    %order_id,
                    "Failed to execute trade. Error: {e:#}"
                );
                update_order_and_match(
                    &mut connection,
                    order_id,
                    MatchState::Failed,
                    OrderState::Failed,
                )?;

                Err(e)
            }
        }
    }

    async fn trade_internal(
        &self,
        trade_params: &TradeParams,
        connection: &mut PgConnection,
    ) -> Result<()> {
        let order_id = trade_params.filled_with.order_id;
        let trader_id = trade_params.pubkey.to_string();
        let order = orders::get_with_id(connection, order_id)?.with_context(|| {
            format!("Could not find order with id {order_id}, trader_id={trader_id}.")
        })?;

        ensure!(
            order.expiry > OffsetDateTime::now_utc(),
            "Can't execute a trade on an expired order"
        );
        ensure!(
            order.order_state == OrderState::Matched,
            "Can't execute trade with in invalid state {:?}",
            order.order_state
        );

        let order_id = trade_params.filled_with.order_id.to_string();
        tracing::info!(trader_id, order_id, "Executing match");

        match self.decide_trade_action(&trade_params.pubkey)? {
            TradeAction::Open => {
                ensure!(
                    self.settings.read().await.allow_opening_positions,
                    "Opening positions is disabled"
                );

                let channel_details = self.get_counterparty_channel(trade_params.pubkey)?;

                let user_channel_id = Uuid::from_u128(channel_details.user_channel_id).to_string();
                let connection = &mut self.pool.get()?;
                let channel =
                    db::channels::get(&user_channel_id, connection)?.with_context(|| {
                        format!(
                            "Couldnt find shadow channel. trader_id={}, user_channel_id={}",
                            trade_params.pubkey, channel_details.user_channel_id
                        )
                    })?;

                let coordinator_leverage = match channel.liquidity_option_id {
                    Some(liquidity_option_id) => {
                        let liquidity_option =
                            db::liquidity_options::get(connection, liquidity_option_id)?;
                        liquidity_option.coordinator_leverage
                    }
                    None => 1.0,
                };

                self.open_position(connection, trade_params, coordinator_leverage, order.stable)
                    .await?;
            }
            TradeAction::Close(channel_id) => {
                let peer_id = trade_params.pubkey;
                tracing::info!(?trade_params, channel_id = %hex::encode(channel_id), %peer_id, "Closing position");

                let closing_price = trade_params.average_execution_price();

                let position = match db::positions::Position::get_position_by_trader(
                    connection,
                    trade_params.pubkey,
                    vec![PositionState::Open],
                )? {
                    Some(position) => position,
                    None => bail!("Failed to find open position : {}", trade_params.pubkey),
                };

                self.close_position(connection, &position, closing_price, channel_id)
                    .await?;
            }
        };

        Ok(())
    }

    #[autometrics]
    async fn open_position(
        &self,
        conn: &mut PgConnection,
        trade_params: &TradeParams,
        coordinator_leverage: f32,
        stable: bool,
    ) -> Result<()> {
        let peer_id = trade_params.pubkey;
        tracing::info!(%peer_id, ?trade_params, "Opening position");

        let margin_trader = margin_trader(trade_params);
        let margin_coordinator = margin_coordinator(trade_params, coordinator_leverage);
        let leverage_trader = trade_params.leverage;
        let total_collateral = margin_coordinator + margin_trader;

        let fee = order_matching_fee_taker(
            trade_params.quantity,
            trade_params.average_execution_price(),
        )
        .to_sat();
        let initial_price = trade_params.filled_with.average_execution_price();

        let coordinator_direction = trade_params.direction.opposite();

        let contract_descriptor = payout_curve::build_contract_descriptor(
            initial_price,
            margin_coordinator,
            margin_trader,
            coordinator_leverage,
            leverage_trader,
            coordinator_direction,
            fee,
            create_rounding_interval(total_collateral),
            trade_params.quantity,
            trade_params.contract_symbol,
        )
        .context("Could not build contract descriptor")?;

        let contract_symbol = trade_params.contract_symbol.label();
        let maturity_time = trade_params.filled_with.expiry_timestamp;
        let maturity_time = maturity_time.unix_timestamp();

        let fee_rate = self.settings.read().await.contract_tx_fee_rate;

        // The contract input to be used for setting up the trade between the trader and the
        // coordinator
        let event_id = format!("{contract_symbol}{maturity_time}");
        tracing::debug!(event_id, "Proposing dlc channel");
        let contract_input = ContractInput {
            offer_collateral: margin_coordinator - fee,
            // the accepting party has do bring in additional margin for the fees
            accept_collateral: margin_trader + fee,
            fee_rate,
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
            .propose_dlc_channel(channel_details.clone(), contract_input)
            .await
            .context("Could not propose dlc channel")?;

        let temporary_contract_id = self
            .inner
            .get_temporary_contract_id_by_sub_channel_id(channel_details.channel_id)
            .context("unable to extract temporary contract id")?;

        // After the dlc channel has been proposed the position can be created. Note, this
        // fixes https://github.com/get10101/10101/issues/537, where the position was created
        // before the dlc was successfully proposed. Although we may still run into
        // inconsistencies e.g. if propose dlc succeeds, but inserting the position and trade
        // into the database doesn't, it is more likely to succeed in the new order.
        // FIXME: Note, we should not create a shadow representation (position) of the DLC struct,
        // but rather imply the state from the DLC.
        self.persist_position_and_trade(
            conn,
            trade_params,
            temporary_contract_id,
            coordinator_leverage,
            stable,
        )
    }

    // Creates a position and a trade from the trade params
    fn persist_position_and_trade(
        &self,
        connection: &mut PgConnection,
        trade_params: &TradeParams,
        temporary_contract_id: ContractId,
        coordinator_leverage: f32,
        stable: bool,
    ) -> Result<()> {
        let liquidation_price = liquidation_price(trade_params);
        let margin_coordinator = margin_coordinator(trade_params, coordinator_leverage);
        let margin_trader = margin_trader(trade_params);

        let average_entry_price = trade_params
            .average_execution_price()
            .to_f32()
            .expect("to fit into f32");

        let new_position = NewPosition {
            contract_symbol: trade_params.contract_symbol,
            trader_leverage: trade_params.leverage,
            quantity: trade_params.quantity,
            direction: trade_params.direction,
            trader: trade_params.pubkey,
            average_entry_price,
            liquidation_price,
            coordinator_margin: margin_coordinator as i64,
            expiry_timestamp: trade_params.filled_with.expiry_timestamp,
            temporary_contract_id,
            trader_margin: margin_trader as i64,
            stable,
        };
        tracing::debug!(?new_position, "Inserting new position into db");

        let position = db::positions::Position::insert(connection, new_position.clone())?;

        db::trades::insert(
            connection,
            NewTrade {
                position_id: position.id,
                contract_symbol: new_position.contract_symbol,
                trader_pubkey: new_position.trader,
                quantity: new_position.quantity,
                trader_leverage: new_position.trader_leverage,
                coordinator_margin: new_position.coordinator_margin,
                direction: new_position.direction,
                average_price: average_entry_price,
            },
        )?;

        Ok(())
    }

    #[autometrics]
    pub async fn close_position(
        &self,
        conn: &mut PgConnection,
        position: &Position,
        closing_price: Decimal,
        channel_id: ChannelId,
    ) -> Result<()> {
        let accept_settlement_amount = position.calculate_settlement_amount(closing_price)?;

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

        db::trades::insert(
            conn,
            NewTrade {
                position_id: position.id,
                contract_symbol: position.contract_symbol,
                trader_pubkey: position.trader,
                quantity: position.quantity,
                trader_leverage: position.trader_leverage,
                coordinator_margin: position.coordinator_margin,
                direction: position.direction.opposite(),
                average_price: closing_price.to_f32().expect("To fit into f32"),
            },
        )?;

        db::positions::Position::set_open_position_to_closing(
            conn,
            position.trader.to_string(),
            closing_price
                .to_f32()
                .expect("Closing price to fit into f32"),
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
        if !self
            .inner
            .dlc_message_handler
            .has_pending_messages_to_process()
        {
            return;
        }

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
                    "Failed to process DLC message: {e:#}"
                );
            }
        }
    }

    fn process_dlc_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %dlc_message_name(&msg),
            "Processing message"
        );

        let resp = match &msg {
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
                .on_sub_channel_message(msg, &node_id)
                .with_context(|| {
                    format!(
                        "Failed to handle {} message from {node_id}",
                        sub_channel_message_name(msg)
                    )
                })?
                .map(Message::SubChannel),
        };

        // TODO(holzeis): It would be nice if dlc messages are also propagated via events, so the
        // receiver can decide what events to process and we can skip this component specific logic
        // here.
        if let Message::Channel(ChannelMessage::RenewFinalize(r)) = &msg {
            self.finalize_rollover(&r.channel_id)?;
        }

        if let Some(msg) = resp {
            tracing::info!(
                to = %node_id,
                kind = %dlc_message_name(&msg),
                "Sending message"
            );

            send_dlc_message(
                &self.inner.dlc_message_handler,
                &self.inner.peer_manager,
                node_id,
                msg,
            );
        }

        Ok(())
    }
}

fn update_order_and_match(
    connection: &mut PgConnection,
    order_id: Uuid,
    match_state: MatchState,
    order_state: OrderState,
) -> Result<()> {
    connection
        .transaction(|connection| {
            matches::set_match_state(connection, order_id, match_state)?;

            orders::set_order_state(connection, order_id, order_state)?;

            diesel::result::QueryResult::Ok(())
        })
        .map_err(|e| anyhow!("Failed to update order and match. Error: {e:#}"))
}

pub enum TradeAction {
    Open,
    Close(ChannelId),
    // Extend,
    // Reduce,
}

fn margin_trader(trade_params: &TradeParams) -> u64 {
    calculate_margin(
        trade_params.average_execution_price(),
        trade_params.quantity,
        trade_params.leverage,
    )
}

fn margin_coordinator(trade_params: &TradeParams, coordinator_leverage: f32) -> u64 {
    calculate_margin(
        trade_params.average_execution_price(),
        trade_params.quantity,
        coordinator_leverage,
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
