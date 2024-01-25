use crate::compute_relative_contracts;
use crate::db;
use crate::decimal_from_f32;
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
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use commons::order_matching_fee_taker;
use commons::MatchState;
use commons::OrderState;
use commons::TradeParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::Connection;
use diesel::PgConnection;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::ContractId;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use dlc_messages::SubChannelMessage;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::ln::ChannelId;
use lightning::util::config::UserConfig;
use ln_dlc_node::node;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::send_dlc_message;
use ln_dlc_node::node::sub_channel_message_name;
use ln_dlc_node::node::RunningNode;
use ln_dlc_node::WalletSettings;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tracing::instrument;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;
use uuid::Uuid;

pub mod connection;
pub mod expired_positions;
pub mod resize;
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
    // Defines if we want to open jit channels
    pub jit_channels_enabled: bool,
    /// Defines the sats/vbyte to be used for all transactions within the sub-channel
    pub contract_tx_fee_rate: u64,
}

impl NodeSettings {
    fn to_wallet_settings(&self) -> WalletSettings {
        WalletSettings {
            max_allowed_tx_fee_rate_when_opening_channel: self
                .max_allowed_tx_fee_rate_when_opening_channel,
            jit_channels_enabled: self.jit_channels_enabled,
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
        self.inner
            .ldk_wallet()
            .update_settings(wallet_settings)
            .await;
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

        match self.decide_trade_action(connection, trade_params)? {
            TradeAction::Open => {
                tracing::debug!(trader_id, order_id, "Opening a new position");

                ensure!(
                    self.settings.read().await.allow_opening_positions,
                    "Opening positions is disabled"
                );

                let coordinator_leverage =
                    self.coordinator_leverage_for_trade(&trade_params.pubkey)?;

                self.open_position(connection, trade_params, coordinator_leverage, order.stable)
                    .await
                    .context("Failed at opening a new position")?;
            }
            TradeAction::Resize(channel_id) => {
                tracing::debug!(trader_id, order_id, "Resizing existing position");
                ensure!(
                    self.settings.read().await.allow_opening_positions,
                    "Resizing positions is disabled"
                );

                self.resize_position(connection, channel_id, trade_params)
                    .await
                    .context("Failed at resizing position")?
            }
            TradeAction::Close(channel_id) => {
                let peer_id = trade_params.pubkey;

                tracing::info!(
                    ?trade_params,
                    channel_id = %hex::encode(channel_id.0),
                    %peer_id,
                    "Closing position"
                );

                let closing_price = trade_params.average_execution_price();

                let position = match db::positions::Position::get_position_by_trader(
                    connection,
                    trade_params.pubkey,
                    vec![PositionState::Open],
                )? {
                    Some(position) => position,
                    None => bail!("Failed to find open position : {}", trade_params.pubkey),
                };

                self.start_closing_position(connection, &position, closing_price, channel_id)
                    .await
                    .context(format!(
                        "Failed at closing the position with id: {}",
                        position.id
                    ))?;
            }
        };

        Ok(())
    }

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
        tracing::debug!(event_id, oracle=%trade_params.filled_with.oracle_pk, "Proposing dlc channel");
        let contract_input = ContractInput {
            offer_collateral: margin_coordinator - fee,
            // the accepting party has do bring in additional margin for the fees
            accept_collateral: margin_trader + fee,
            fee_rate,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![trade_params.filled_with.oracle_pk],
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
            coordinator_leverage,
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
                dlc_expiry_timestamp: Some(trade_params.filled_with.expiry_timestamp),
            },
        )?;

        Ok(())
    }

    pub async fn start_closing_position(
        &self,
        conn: &mut PgConnection,
        position: &Position,
        closing_price: Decimal,
        channel_id: ChannelId,
    ) -> Result<()> {
        let accept_settlement_amount =
            position.calculate_accept_settlement_amount(closing_price)?;

        tracing::debug!(
            ?position,
            channel_id = %hex::encode(channel_id.0),
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
                // A closing trade does not require an expiry timestamp for the DLC, because the DLC
                // is being _removed_.
                dlc_expiry_timestamp: None,
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

    #[instrument(fields(position_id = position.id, trader_id = position.trader.to_string()),skip(self, conn, position))]
    pub fn finalize_closing_position(
        &self,
        conn: &mut PgConnection,
        position: Position,
    ) -> Result<()> {
        let trader_id = position.trader.to_string();
        tracing::debug!(?position, trader_id, "Finalize closing position",);

        let position_id = position.id;
        let temporary_contract_id = match position.temporary_contract_id {
            None => {
                tracing::error!("Position does not have temporary contract id");
                bail!("Position with id {position_id} with trader {trader_id} does not have temporary contract id");
            }
            Some(temporary_contract_id) => temporary_contract_id,
        };

        let contract = match self.inner.get_closed_contract(temporary_contract_id) {
            Ok(Some(closed_contract)) => closed_contract,
            Ok(None) => {
                tracing::error!("Subchannel not closed yet, skipping");
                bail!("Subchannel not closed for position {position_id} and trader {trader_id}");
            }
            Err(e) => {
                tracing::error!("Failed to get closed contract from DLC manager storage: {e:#}");
                bail!(e);
            }
        };

        tracing::debug!(
            ?position,
            "Setting position to closed to match the contract state."
        );

        if let Err(e) = db::positions::Position::set_position_to_closed_with_pnl(
            conn,
            position.id,
            contract.pnl,
        ) {
            tracing::error!(
                temporary_contract_id=%temporary_contract_id.to_hex(),
                pnl=contract.pnl,
                "Failed to set position to closed: {e:#}"
            )
        }
        Ok(())
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
    /// caller to resize the position.
    pub fn decide_trade_action(
        &self,
        conn: &mut PgConnection,
        trade_params: &TradeParams,
    ) -> Result<TradeAction> {
        let trader_peer_id = trade_params.pubkey;

        let subchannel = match self.inner.get_dlc_channel_signed(&trader_peer_id)? {
            None => return Ok(TradeAction::Open),
            Some(subchannel) => subchannel,
        };

        let position = db::positions::Position::get_position_by_trader(
            conn,
            trader_peer_id,
            vec![PositionState::Open],
        )?
        .with_context(|| format!("Failed to find open position with peer {trader_peer_id}"))?;

        let position_contracts = {
            let contracts = decimal_from_f32(position.quantity);

            compute_relative_contracts(contracts, &position.direction)
        };

        let trade_contracts = {
            let contracts = decimal_from_f32(trade_params.quantity);

            compute_relative_contracts(contracts, &trade_params.direction)
        };

        let action = if position_contracts + trade_contracts == Decimal::ZERO {
            TradeAction::Close(subchannel.channel_id)
        } else {
            TradeAction::Resize(subchannel.channel_id)
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

        if let Message::SubChannel(SubChannelMessage::Finalize(finalize)) = &msg {
            let channel_id_hex_string = finalize.channel_id.to_hex();
            tracing::info!(
                channel_id = channel_id_hex_string,
                node_id = node_id.to_string(),
                "Subchannel open protocol was finalized"
            );
            let mut connection = self.pool.get()?;
            db::positions::Position::update_proposed_position(
                &mut connection,
                node_id.to_string(),
                PositionState::Open,
            )?;
        }

        if let Message::SubChannel(SubChannelMessage::CloseFinalize(msg)) = &msg {
            let mut connection = self.pool.get()?;
            match db::positions::Position::get_position_by_trader(
                &mut connection,
                node_id,
                vec![
                    // the price doesn't matter here
                    PositionState::Closing { closing_price: 0.0 },
                    PositionState::Resizing,
                ],
            )? {
                None => {
                    tracing::warn!(
                        channel_id = msg.channel_id.to_hex(),
                        "No position found to finalize"
                    );
                }
                Some(position) => match position.position_state {
                    PositionState::Closing { .. } => {
                        self.finalize_closing_position(&mut connection, position)?;
                    }
                    PositionState::Resizing => {
                        self.continue_position_resizing(node_id, position)?;
                    }
                    state => {
                        // this should never happen because we are only loading specific states
                        tracing::error!(
                            channel_id = msg.channel_id.to_hex(),
                            position_id = position.id,
                            position_state = ?state,
                            "Position was in unexpected state when trying to finalize the subchannel"
                        );
                    }
                },
            }
        }

        if let Message::SubChannel(SubChannelMessage::Reject(reject)) = &msg {
            let channel_id_hex = reject.channel_id.to_hex();
            tracing::warn!(channel_id = channel_id_hex, "Subchannel offer was rejected");
            let mut connection = self.pool.get()?;
            match db::positions::Position::get_position_by_trader(
                &mut connection,
                node_id,
                vec![
                    PositionState::Proposed,
                    PositionState::ResizeOpeningSubchannelProposed,
                ],
            )? {
                None => {
                    tracing::warn!("No position found to be updated")
                }
                Some(position) => {
                    let updated_state = match position.position_state {
                        PositionState::Proposed => PositionState::Failed,
                        PositionState::ResizeOpeningSubchannelProposed => PositionState::Open,
                        state => {
                            // This should not happen because we only load these two states above
                            bail!("Position was in unexpected state {state:?}.");
                        }
                    };
                    db::positions::Position::update_proposed_position(
                        &mut connection,
                        node_id.to_string(),
                        updated_state,
                    )?;
                }
            }
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

    fn coordinator_leverage_for_trade(&self, counterparty_peer_id: &PublicKey) -> Result<f32> {
        let mut conn = self.pool.get()?;

        let channel_details = self.get_counterparty_channel(*counterparty_peer_id)?;
        let user_channel_id = Uuid::from_u128(channel_details.user_channel_id).to_string();
        let channel = db::channels::get(&user_channel_id, &mut conn)?.with_context(|| {
            format!("Couldn't find shadow channel with user channel ID {user_channel_id}",)
        })?;
        let leverage_coordinator = match channel.liquidity_option_id {
            Some(liquidity_option_id) => {
                let liquidity_option = db::liquidity_options::get(&mut conn, liquidity_option_id)?;
                liquidity_option.coordinator_leverage
            }
            None => 1.0,
        };

        Ok(leverage_coordinator)
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
    Resize(ChannelId),
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
