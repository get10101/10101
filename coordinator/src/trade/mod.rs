use crate::compute_relative_contracts;
use crate::db;
use crate::decimal_from_f32;
use crate::dlc_protocol;
use crate::dlc_protocol::DlcProtocolType;
use crate::dlc_protocol::ProtocolId;
use crate::message::OrderbookMessage;
use crate::node::Node;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use crate::payout_curve;
use crate::position::models::NewPosition;
use crate::position::models::Position;
use crate::position::models::PositionState;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::SignedAmount;
use commons::MatchState;
use commons::Message;
use commons::OrderState;
use commons::TradeAndChannelParams;
use commons::TradeParams;
use diesel::Connection;
use diesel::PgConnection;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::ContractId;
use dlc_manager::DlcChannelId;
use lightning::chain::chaininterface::ConfirmationTarget;
use ln_dlc_node::bitcoin_conversion::to_secp_pk_29;
use ln_dlc_node::bitcoin_conversion::to_xonly_pk_29;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_node::node::signed_channel_state_name;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use trade::cfd::calculate_long_liquidation_price;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_pnl;
use trade::cfd::calculate_short_liquidation_price;
use trade::Direction;
use uuid::Uuid;

pub mod models;
pub mod websocket;

enum TradeAction {
    OpenDlcChannel,
    OpenPosition {
        channel_id: DlcChannelId,
        own_payout: u64,
        counter_payout: u64,
    },
    ClosePosition {
        channel_id: DlcChannelId,
        position: Box<Position>,
    },
    ResizePosition {
        channel_id: DlcChannelId,
        position: Box<Position>,
        resize_action: ResizeAction,
    },
}

#[derive(Debug, Clone, Copy)]
enum ResizeAction {
    Increase {
        /// Absolute number of contracts we increase the position by.
        contracts: Decimal,
        average_execution_price: Decimal,
    },
    Decrease {
        /// Absolute number of contracts we decrease the position by.
        contracts: Decimal,
        average_execution_price: Decimal,
    },
    ChangeDirection {
        /// The sign determines the new direction.
        contracts_new_direction: Decimal,
        average_execution_price: Decimal,
    },
}

pub struct TradeExecutor {
    node: Node,
    notifier: mpsc::Sender<OrderbookMessage>,
}

impl TradeExecutor {
    pub fn new(node: Node, notifier: mpsc::Sender<OrderbookMessage>) -> Self {
        Self { node, notifier }
    }

    pub async fn execute(&self, params: &TradeAndChannelParams) {
        let trader_id = params.trade_params.pubkey;
        let order_id = params.trade_params.filled_with.order_id;

        match self.execute_internal(params).await {
            Ok(()) => {
                tracing::info!(
                    %trader_id,
                    %order_id,
                    "Successfully processed match, setting match to Filled"
                );

                if let Err(e) =
                    self.update_order_and_match(order_id, MatchState::Filled, OrderState::Taken)
                {
                    tracing::error!(
                        %trader_id,
                        %order_id,
                        "Failed to update order and match state. Error: {e:#}"
                    );
                }

                // Everything has been processed successfully, we can safely send the last dlc
                // message, that has been stored before.
                self.node
                    .inner
                    .event_handler
                    .publish(NodeEvent::SendLastDlcMessage { peer: trader_id });
            }
            Err(e) => {
                tracing::error!(%trader_id, %order_id,"Failed to execute trade. Error: {e:#}");

                if let Err(e) =
                    self.update_order_and_match(order_id, MatchState::Failed, OrderState::Failed)
                {
                    tracing::error!(%trader_id, %order_id, "Failed to update order and match: {e}");
                };

                let message = OrderbookMessage::TraderMessage {
                    trader_id,
                    message: Message::TradeError {
                        order_id,
                        error: e.into(),
                    },
                    notification: None,
                };
                if let Err(e) = self.notifier.send(message).await {
                    tracing::debug!("Failed to notify trader. Error: {e:#}");
                }
            }
        };
    }

    /// Execute a trade action according to the coordinator's current trading status with the
    /// trader.
    ///
    /// We look for a pre-existing position with the trader and execute accordingly:
    ///
    /// 0. If no DLC channel is found, we open a DLC channel (with the position included).
    ///
    /// 1. If a position of equal quantity and opposite direction is found, we close the position.
    ///
    /// 2. If no position is found, we open a position.
    ///
    /// 3. If a position of differing quantity is found, we resize the position.
    async fn execute_internal(&self, params: &TradeAndChannelParams) -> Result<()> {
        let mut connection = self.node.pool.get()?;

        let order_id = params.trade_params.filled_with.order_id;
        let trader_id = params.trade_params.pubkey;
        let order =
            orders::get_with_id(&mut connection, order_id)?.context("Could not find order")?;
        let is_stable_order = order.stable;

        ensure!(
            order.expiry > OffsetDateTime::now_utc(),
            "Can't execute a trade on an expired order"
        );
        ensure!(
            order.order_state == OrderState::Matched,
            "Can't execute trade with in invalid state {:?}",
            order.order_state
        );

        tracing::info!(%trader_id, %order_id, "Executing match");

        let trade_action = self.determine_trade_action(&mut connection, params).await?;

        ensure!(
            matches!(trade_action, TradeAction::ClosePosition { .. })
                || self.node.settings.read().await.allow_opening_positions,
            "Trading is disabled except for closing positions"
        );

        match trade_action {
            TradeAction::OpenDlcChannel => {
                let collateral_reserve_coordinator = params
                    .coordinator_reserve
                    .context("Missing coordinator collateral reserve")?;
                let collateral_reserve_trader = params
                    .trader_reserve
                    .context("Missing trader collateral reserve")?;

                self.open_dlc_channel(
                    &mut connection,
                    &params.trade_params,
                    collateral_reserve_coordinator,
                    collateral_reserve_trader,
                    is_stable_order,
                )
                .await
                .context("Failed to open DLC channel")?;
            }
            TradeAction::OpenPosition {
                channel_id,
                own_payout,
                counter_payout,
            } => self
                .open_position(
                    &mut connection,
                    channel_id,
                    &params.trade_params,
                    own_payout,
                    counter_payout,
                    is_stable_order,
                )
                .await
                .context("Failed to open new position")?,
            TradeAction::ClosePosition {
                channel_id,
                position,
            } => self
                .start_closing_position(
                    &mut connection,
                    &position,
                    &params.trade_params,
                    channel_id,
                )
                .await
                .with_context(|| format!("Failed to close position {}", position.id))?,
            TradeAction::ResizePosition {
                channel_id,
                position,
                resize_action,
            } => self
                .resize_position(
                    &mut connection,
                    channel_id,
                    &position,
                    &params.trade_params,
                    resize_action,
                )
                .await
                .with_context(|| format!("Failed to resize position {}", position.id))?,
        };

        Ok(())
    }

    async fn open_dlc_channel(
        &self,
        conn: &mut PgConnection,
        trade_params: &TradeParams,
        collateral_reserve_coordinator: Amount,
        collateral_reserve_trader: Amount,
        stable: bool,
    ) -> Result<()> {
        let peer_id = trade_params.pubkey;

        let leverage_trader = trade_params.leverage;
        let leverage_coordinator = coordinator_leverage_for_trade(&trade_params.pubkey)?;

        let margin_trader = margin_trader(trade_params);
        let margin_coordinator = margin_coordinator(trade_params, leverage_coordinator);

        let order_matching_fee = trade_params.order_matching_fee().to_sat();

        // The coordinator gets the `order_matching_fee` directly in the collateral reserve.
        let collateral_reserve_with_fee_coordinator =
            collateral_reserve_coordinator.to_sat() + order_matching_fee;
        let collateral_reserve_trader = collateral_reserve_trader.to_sat();

        let initial_price = trade_params.filled_with.average_execution_price();

        let coordinator_direction = trade_params.direction.opposite();

        tracing::info!(
            %peer_id,
            order_id = %trade_params.filled_with.order_id,
            ?trade_params,
            leverage_coordinator,
            margin_coordinator_sat = %margin_coordinator,
            margin_trader_sat = %margin_trader,
            order_matching_fee_sat = %order_matching_fee,
            collateral_reserve_with_fee_coordinator = %collateral_reserve_with_fee_coordinator,
            collateral_reserve_trader = %collateral_reserve_trader,
            "Opening DLC channel and position"
        );

        let contract_descriptor = payout_curve::build_contract_descriptor(
            initial_price,
            margin_coordinator,
            margin_trader,
            leverage_coordinator,
            leverage_trader,
            coordinator_direction,
            collateral_reserve_with_fee_coordinator,
            collateral_reserve_trader,
            trade_params.quantity,
            trade_params.contract_symbol,
        )
        .context("Could not build contract descriptor")?;

        let contract_symbol = trade_params.contract_symbol.label();
        let maturity_time = trade_params.filled_with.expiry_timestamp;
        let maturity_time = maturity_time.unix_timestamp();

        let sats_per_vbyte = self
            .node
            .inner
            .fee_rate_estimator
            .get(ConfirmationTarget::Normal)
            .as_sat_per_vb()
            .round();
        // This fee rate is used to construct the fund and CET transactions.
        let fee_rate = Decimal::try_from(sats_per_vbyte)?
            .to_u64()
            .context("failed to convert to u64")?;

        // The contract input to be used for setting up the trade between the trader and the
        // coordinator.
        let event_id = format!("{contract_symbol}{maturity_time}");

        let contract_input = ContractInput {
            offer_collateral: margin_coordinator + collateral_reserve_coordinator.to_sat(),
            // The accept party has do bring additional collateral to pay for the
            // `order_matching_fee`.
            accept_collateral: margin_trader + collateral_reserve_trader + order_matching_fee,
            fee_rate,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![to_xonly_pk_29(trade_params.filled_with.oracle_pk)],
                    event_id: event_id.clone(),
                    threshold: 1,
                },
            }],
        };

        let protocol_id = ProtocolId::new();

        tracing::debug!(
            %protocol_id,
            event_id,
            oracle=%trade_params.filled_with.oracle_pk,
            "Proposing DLC channel"
        );

        let (temporary_contract_id, temporary_channel_id) = self
            .node
            .inner
            .propose_dlc_channel(contract_input, trade_params.pubkey, protocol_id.into())
            .await
            .context("Could not propose DLC channel")?;

        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.node.pool.clone());
        protocol_executor.start_dlc_protocol(
            protocol_id,
            None,
            &temporary_contract_id,
            &temporary_channel_id,
            DlcProtocolType::open_channel(trade_params, protocol_id),
        )?;

        // After the DLC channel has been proposed the position can be created. This fixes
        // https://github.com/get10101/10101/issues/537, where the position was created before the
        // DLC was successfully proposed.
        //
        // Athough we can still run into inconsistencies (e.g. if `propose_dlc_channel` succeeds,
        // but `persist_position_and_trade` doesn't), we are more likely to succeed with the new
        // order.
        //
        // FIXME: We should not create a shadow representation (position) of the DLC struct, but
        // rather imply the state from the DLC.
        //
        // TODO(holzeis): The position should only get created after the dlc protocol has finished
        // successfully.
        self.persist_position(
            conn,
            trade_params,
            temporary_contract_id,
            leverage_coordinator,
            stable,
            Amount::from_sat(order_matching_fee),
        )
        .await
    }

    async fn open_position(
        &self,
        conn: &mut PgConnection,
        dlc_channel_id: DlcChannelId,
        trade_params: &TradeParams,
        coordinator_dlc_channel_collateral: u64,
        trader_dlc_channel_collateral: u64,
        stable: bool,
    ) -> Result<()> {
        let peer_id = trade_params.pubkey;

        tracing::info!(
            %peer_id,
            order_id = %trade_params.filled_with.order_id,
            channel_id = %hex::encode(dlc_channel_id),
            ?trade_params,
            "Opening position"
        );

        let initial_price = trade_params.filled_with.average_execution_price();

        let leverage_coordinator = coordinator_leverage_for_trade(&trade_params.pubkey)?;
        let leverage_trader = trade_params.leverage;

        let margin_coordinator = margin_coordinator(trade_params, leverage_coordinator);
        let margin_trader = margin_trader(trade_params);

        let order_matching_fee = trade_params.order_matching_fee().to_sat();

        let coordinator_direction = trade_params.direction.opposite();

        // How many coins the coordinator will keep outside of the bet. They still go in the DLC
        // channel, but the payout will be at least this much for the coordinator.
        //
        // TODO: Do we want to let the coordinator use accrued order-matching fees as margin?
        // Probably not.
        let coordinator_collateral_reserve = (coordinator_dlc_channel_collateral
            + order_matching_fee)
            .checked_sub(margin_coordinator)
            .with_context(|| {
                format!(
                    "Coordinator cannot trade with more than their total collateral in the \
                     DLC channel: margin ({}) > collateral ({}) + order_matching_fee ({})",
                    margin_coordinator, coordinator_dlc_channel_collateral, order_matching_fee
                )
            })?;

        // How many coins the trader will keep outside of the bet. They still go in the DLC channel,
        // but the payout will be at least this much for the coordinator.
        let trader_collateral_reserve = trader_dlc_channel_collateral
            .checked_sub(order_matching_fee)
            .and_then(|collateral| collateral.checked_sub(margin_trader))
            .with_context(|| {
                format!(
                    "Trader cannot trade with more than their total collateral in the \
                     DLC channel: margin ({}) + order_matching_fee ({}) > collateral ({})",
                    margin_trader, order_matching_fee, trader_dlc_channel_collateral
                )
            })?;

        tracing::debug!(
            %peer_id,
            order_id = %trade_params.filled_with.order_id,
            leverage_coordinator,
            margin_coordinator_sat = %margin_coordinator,
            margin_trader_sat = %margin_trader,
            coordinator_collateral_reserve_sat = %coordinator_collateral_reserve,
            trader_collateral_reserve_sat = %trader_collateral_reserve,
            order_matching_fee_sat = %order_matching_fee,
            "DLC channel update parameters"
        );

        let contract_descriptor = payout_curve::build_contract_descriptor(
            initial_price,
            margin_coordinator,
            margin_trader,
            leverage_coordinator,
            leverage_trader,
            coordinator_direction,
            coordinator_collateral_reserve,
            trader_collateral_reserve,
            trade_params.quantity,
            trade_params.contract_symbol,
        )
        .context("Could not build contract descriptor")?;

        let contract_symbol = trade_params.contract_symbol.label();
        let maturity_time = trade_params.filled_with.expiry_timestamp;
        let maturity_time = maturity_time.unix_timestamp();

        let sats_per_vbyte = self
            .node
            .inner
            .fee_rate_estimator
            .get(ConfirmationTarget::Normal)
            .as_sat_per_vb()
            .round();
        // This fee rate is actually ignored since the fee reserve is defined when the channel is
        // first opened.
        let fee_rate = Decimal::try_from(sats_per_vbyte)?
            .to_u64()
            .context("failed to convert to u64")?;

        // The contract input to be used for setting up the trade between the trader and the
        // coordinator.
        let event_id = format!("{contract_symbol}{maturity_time}");

        tracing::debug!(
            event_id,
            oracle=%trade_params.filled_with.oracle_pk,
            "Proposing DLC channel update"
        );

        let contract_input = ContractInput {
            offer_collateral: coordinator_dlc_channel_collateral,
            accept_collateral: trader_dlc_channel_collateral,
            fee_rate,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![to_xonly_pk_29(trade_params.filled_with.oracle_pk)],
                    event_id,
                    threshold: 1,
                },
            }],
        };

        let protocol_id = ProtocolId::new();
        let channel = self.node.inner.get_dlc_channel_by_id(&dlc_channel_id)?;
        let previous_id = match channel.get_reference_id() {
            Some(reference_id) => Some(ProtocolId::try_from(reference_id)?),
            None => None,
        };

        let temporary_contract_id = self
            .node
            .inner
            .propose_dlc_channel_update(&dlc_channel_id, contract_input, protocol_id.into())
            .await
            .context("Could not propose DLC channel update")?;

        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.node.pool.clone());
        protocol_executor.start_dlc_protocol(
            protocol_id,
            previous_id,
            &temporary_contract_id,
            &channel.get_id(),
            DlcProtocolType::open_position(trade_params, protocol_id),
        )?;

        // TODO(holzeis): The position should only get created after the dlc protocol has finished
        // successfully.
        self.persist_position(
            conn,
            trade_params,
            temporary_contract_id,
            leverage_coordinator,
            stable,
            Amount::from_sat(order_matching_fee),
        )
        .await
    }

    async fn resize_position(
        &self,
        conn: &mut PgConnection,
        dlc_channel_id: DlcChannelId,
        position: &Position,
        trade_params: &TradeParams,
        resize_action: ResizeAction,
    ) -> Result<()> {
        if !self.node.inner.is_dlc_channel_confirmed(&dlc_channel_id)? {
            bail!("Underlying DLC channel not yet confirmed");
        }

        let peer_id = trade_params.pubkey;

        let maintenance_margin_rate = {
            Decimal::try_from(self.node.settings.read().await.maintenance_margin_rate)
                .expect("to fit")
        };

        tracing::info!(
            %peer_id,
            order_id = %trade_params.filled_with.order_id,
            channel_id = %hex::encode(dlc_channel_id),
            ?resize_action,
            ?position,
            ?trade_params,
            "Resizing position"
        );

        let order_matching_fee = trade_params.order_matching_fee();

        // The leverage does not change when we resize a position.

        let original_coordinator_collateral_reserve = self
            .node
            .inner
            .get_dlc_channel_usable_balance(&dlc_channel_id)?;
        let original_trader_collateral_reserve = self
            .node
            .inner
            .get_dlc_channel_usable_balance_counterparty(&dlc_channel_id)?;

        let resized_position = apply_resize_to_position(
            resize_action,
            position,
            original_coordinator_collateral_reserve,
            original_trader_collateral_reserve,
            order_matching_fee,
            maintenance_margin_rate,
        )?;

        let leverage_coordinator = position.coordinator_leverage;
        let leverage_trader = position.trader_leverage;

        tracing::debug!(
            %peer_id,
            order_id = %trade_params.filled_with.order_id,
            leverage_coordinator,
            leverage_trader,
            %order_matching_fee,
            ?resized_position,
            "DLC channel update parameters"
        );

        let ResizedPosition {
            contracts,
            coordinator_direction,
            average_execution_price,
            coordinator_liquidation_price,
            trader_liquidation_price,
            margin_coordinator,
            margin_trader,
            collateral_reserve_coordinator,
            collateral_reserve_trader,
            realized_pnl,
        } = resized_position;

        let contract_descriptor = payout_curve::build_contract_descriptor(
            average_execution_price,
            margin_coordinator.to_sat(),
            margin_trader.to_sat(),
            leverage_coordinator,
            leverage_trader,
            coordinator_direction,
            collateral_reserve_coordinator.to_sat(),
            collateral_reserve_trader.to_sat(),
            contracts.to_f32().expect("to fit"),
            trade_params.contract_symbol,
        )
        .context("Could not build contract descriptor")?;

        let contract_symbol = trade_params.contract_symbol.label();
        let expiry_timestamp = trade_params.filled_with.expiry_timestamp;
        let expiry_unix_timestamp = expiry_timestamp.unix_timestamp();

        let sats_per_vbyte = self
            .node
            .inner
            .fee_rate_estimator
            .get(ConfirmationTarget::Normal)
            .as_sat_per_vb()
            .round();
        // This fee rate is actually ignored since the fee reserve is defined when the channel is
        // first opened.
        let fee_rate = Decimal::try_from(sats_per_vbyte)?
            .to_u64()
            .context("failed to convert to u64")?;

        // The contract input to be used for setting up the trade between the trader and the
        // coordinator.
        let event_id = format!("{contract_symbol}{expiry_unix_timestamp}");

        tracing::debug!(
            event_id,
            oracle=%trade_params.filled_with.oracle_pk,
            "Proposing DLC channel update"
        );

        let contract_input = ContractInput {
            offer_collateral: margin_coordinator.to_sat() + collateral_reserve_coordinator.to_sat(),
            accept_collateral: margin_trader.to_sat() + collateral_reserve_trader.to_sat(),
            fee_rate,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![to_xonly_pk_29(trade_params.filled_with.oracle_pk)],
                    event_id,
                    threshold: 1,
                },
            }],
        };

        let protocol_id = ProtocolId::new();
        let channel = self.node.inner.get_dlc_channel_by_id(&dlc_channel_id)?;
        let previous_id = match channel.get_reference_id() {
            Some(reference_id) => Some(ProtocolId::try_from(reference_id)?),
            None => None,
        };

        let temporary_contract_id = self
            .node
            .inner
            .propose_dlc_channel_update(&dlc_channel_id, contract_input, protocol_id.into())
            .await
            .context("Could not propose DLC channel update")?;

        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.node.pool.clone());
        protocol_executor.start_dlc_protocol(
            protocol_id,
            previous_id,
            &temporary_contract_id,
            &channel.get_id(),
            DlcProtocolType::resize_position(trade_params, protocol_id, realized_pnl),
        )?;

        db::positions::Position::set_position_to_resizing(
            conn,
            peer_id,
            temporary_contract_id,
            contracts,
            coordinator_direction.opposite(),
            margin_trader,
            margin_coordinator,
            average_execution_price,
            expiry_timestamp,
            coordinator_liquidation_price,
            trader_liquidation_price,
            realized_pnl,
            order_matching_fee,
        )?;

        Ok(())
    }

    async fn persist_position(
        &self,
        connection: &mut PgConnection,
        trade_params: &TradeParams,
        temporary_contract_id: ContractId,
        coordinator_leverage: f32,
        stable: bool,
        order_matching_fees: Amount,
    ) -> Result<()> {
        let price = trade_params.average_execution_price();
        let maintenance_margin_rate = { self.node.settings.read().await.maintenance_margin_rate };
        let maintenance_margin_rate =
            Decimal::try_from(maintenance_margin_rate).expect("to fit into decimal");

        let trader_liquidation_price = liquidation_price(
            price,
            Decimal::try_from(coordinator_leverage).expect("to fit into decimal"),
            trade_params.direction,
            maintenance_margin_rate,
        );

        let coordinator_liquidation_price = liquidation_price(
            price,
            Decimal::try_from(coordinator_leverage).expect("to fit into decimal"),
            trade_params.direction.opposite(),
            maintenance_margin_rate,
        );

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
            trader_direction: trade_params.direction,
            trader: trade_params.pubkey,
            average_entry_price,
            trader_liquidation_price,
            coordinator_liquidation_price,
            coordinator_margin: margin_coordinator as i64,
            expiry_timestamp: trade_params.filled_with.expiry_timestamp,
            temporary_contract_id,
            coordinator_leverage,
            trader_margin: margin_trader as i64,
            stable,
            order_matching_fees,
        };
        tracing::debug!(?new_position, "Inserting new position into db");

        // TODO(holzeis): We should only create the position once the dlc protocol finished
        // successfully.
        db::positions::Position::insert(connection, new_position.clone())?;

        Ok(())
    }

    pub async fn start_closing_position(
        &self,
        conn: &mut PgConnection,
        position: &Position,
        trade_params: &TradeParams,
        channel_id: DlcChannelId,
    ) -> Result<()> {
        if !self.node.inner.is_dlc_channel_confirmed(&channel_id)? {
            bail!("Underlying DLC channel not yet confirmed");
        }

        let closing_price = trade_params.average_execution_price();
        let position_settlement_amount_coordinator = position
            .calculate_coordinator_settlement_amount(
                closing_price,
                trade_params.order_matching_fee(),
            )?;

        let collateral_reserve_coordinator = self
            .node
            .inner
            .get_dlc_channel_usable_balance(&channel_id)?;
        let dlc_channel_settlement_amount_coordinator =
            position_settlement_amount_coordinator + collateral_reserve_coordinator.to_sat();

        let protocol_id = ProtocolId::new();
        tracing::info!(
            %protocol_id,
            ?position,
            channel_id = %hex::encode(channel_id),
            %position_settlement_amount_coordinator,
            ?collateral_reserve_coordinator,
            %dlc_channel_settlement_amount_coordinator,
            trader_peer_id = %position.trader,
            "Closing position by settling DLC channel off-chain",
        );

        let total_collateral = self
            .node
            .inner
            .signed_dlc_channel_total_collateral(&channel_id)?;

        let settlement_amount_trader = total_collateral
            .to_sat()
            .checked_sub(dlc_channel_settlement_amount_coordinator)
            .unwrap_or_default();

        let channel = self.node.inner.get_dlc_channel_by_id(&channel_id)?;
        let contract_id = channel.get_contract_id().context("missing contract id")?;
        let previous_id = match channel.get_reference_id() {
            Some(reference_id) => Some(ProtocolId::try_from(reference_id)?),
            None => None,
        };

        self.node
            .inner
            .propose_dlc_channel_collaborative_settlement(
                &channel_id,
                settlement_amount_trader,
                protocol_id.into(),
            )
            .await?;

        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.node.pool.clone());
        protocol_executor.start_dlc_protocol(
            protocol_id,
            previous_id,
            &contract_id,
            &channel.get_id(),
            DlcProtocolType::settle(trade_params, protocol_id),
        )?;

        db::positions::Position::set_open_position_to_closing(
            conn,
            &position.trader,
            Some(closing_price),
        )?;

        Ok(())
    }

    fn update_order_and_match(
        &self,
        order_id: Uuid,
        match_state: MatchState,
        order_state: OrderState,
    ) -> Result<()> {
        let mut connection = self.node.pool.get()?;
        connection
            .transaction(|connection| {
                matches::set_match_state(connection, order_id, match_state)?;

                orders::set_order_state(connection, order_id, order_state)?;

                diesel::result::QueryResult::Ok(())
            })
            .map_err(|e| anyhow!("Failed to update order and match. Error: {e:#}"))
    }

    async fn determine_trade_action(
        &self,
        connection: &mut PgConnection,
        params: &TradeAndChannelParams,
    ) -> Result<TradeAction> {
        let trader_id = params.trade_params.pubkey;

        let trade_action = match self
            .node
            .inner
            .get_signed_dlc_channel_by_counterparty(&trader_id)?
        {
            None => {
                ensure!(
                    !self
                        .node
                        .inner
                        .list_dlc_channels()?
                        .iter()
                        .filter(|c| c.get_counter_party_id() == to_secp_pk_29(trader_id))
                        .any(|c| matches!(c, Channel::Offered(_) | Channel::Accepted(_))),
                    "Previous DLC Channel offer still pending."
                );

                TradeAction::OpenDlcChannel
            }
            Some(SignedChannel {
                channel_id,
                state:
                    SignedChannelState::Settled {
                        own_payout,
                        counter_payout,
                        ..
                    },
                ..
            }) => TradeAction::OpenPosition {
                channel_id,
                own_payout,
                counter_payout,
            },
            Some(SignedChannel {
                state: SignedChannelState::Established { .. },
                channel_id,
                ..
            }) => {
                let trade_params = &params.trade_params;

                let position = db::positions::Position::get_position_by_trader(
                    connection,
                    trader_id,
                    vec![PositionState::Open],
                )?
                .context("Failed to find open position")?;

                let position_contracts = {
                    let contracts = decimal_from_f32(position.quantity);

                    compute_relative_contracts(contracts, &position.trader_direction)
                };

                let trade_contracts = {
                    let contracts = decimal_from_f32(trade_params.quantity);

                    compute_relative_contracts(contracts, &trade_params.direction)
                };

                let average_execution_price = trade_params.filled_with.average_execution_price();

                match (position_contracts, trade_contracts) {
                    // If they cancel out, we are closing the position.
                    (p, t) if (p + t).is_zero() => TradeAction::ClosePosition {
                        channel_id,
                        position: Box::new(position),
                    },
                    // If the signs are the same, we are increasing the position.
                    (p, t) if (p * t).is_sign_positive() => TradeAction::ResizePosition {
                        channel_id,
                        position: Box::new(position),
                        resize_action: ResizeAction::Increase {
                            contracts: t.abs(),
                            average_execution_price,
                        },
                    },
                    // If the signs are different and the trade contracts are lower than the
                    // position contracts, we are decreasing the position.
                    (p, t) if (p * t).is_sign_negative() && t.abs() < p.abs() => {
                        TradeAction::ResizePosition {
                            channel_id,
                            position: Box::new(position),
                            resize_action: ResizeAction::Decrease {
                                contracts: t.abs(),
                                average_execution_price,
                            },
                        }
                    }
                    // If the signs are different and the trade contracts are greater than the
                    // position contracts, we are changing position direction.
                    (p, t) => {
                        let contracts_new_direction = t.abs() - p.abs();
                        let contracts_new_direction = contracts_new_direction.abs();

                        TradeAction::ResizePosition {
                            channel_id,
                            position: Box::new(position),
                            resize_action: ResizeAction::ChangeDirection {
                                contracts_new_direction,
                                average_execution_price,
                            },
                        }
                    }
                }
            }
            Some(signed_channel) => {
                bail!(
                    "Cannot trade with DLC channel in state {}",
                    signed_channel_state_name(&signed_channel)
                );
            }
        };

        Ok(trade_action)
    }
}

/// The [`Position`] values that can change after a resize.
#[derive(Debug, PartialEq)]
struct ResizedPosition {
    contracts: Decimal,
    coordinator_direction: Direction,
    average_execution_price: Decimal,
    coordinator_liquidation_price: Decimal,
    trader_liquidation_price: Decimal,
    margin_coordinator: Amount,
    margin_trader: Amount,
    collateral_reserve_coordinator: Amount,
    collateral_reserve_trader: Amount,
    realized_pnl: Option<SignedAmount>,
}

// We want to recompute the `ContractInput` from scratch. But we need to make sure that the
// computed values are sensible: the app and coordinator margin must be within certain
// bounds.
//
// The `accumulated_order_matching_fees` sets the lower bound for the
// `coordinator_collateral_reserve`.
//
// Cases:
//
// 1. Increasing position: Capped by collateral reserves. Remember that the
// `accumulated_order_matching_fees` cannot be used.
//
// 2. Reducing position: Converts some unrealised PNL into realised PNL. Only requirement is
// that the trader should be able to pay for the order-matching fee (what to do if they
// can't afford it?! I suppose we liquidate).
//
// 3. Change direction: What if we close the model position and create a new one? The
// underlying protocol will still be renew.
fn apply_resize_to_position(
    resize_action: ResizeAction,
    position: &Position,
    original_coordinator_collateral_reserve: Amount,
    original_trader_collateral_reserve: Amount,
    order_matching_fee: Amount,
    maintenance_margin_rate: Decimal,
) -> Result<ResizedPosition> {
    let resized_position = match resize_action {
        ResizeAction::Increase {
            contracts,
            average_execution_price: order_execution_price,
        } => {
            let order_contracts = contracts;

            let extra_margin_coordinator = Amount::from_sat(calculate_margin(
                order_execution_price,
                order_contracts.to_f32().expect("to fit"),
                position.coordinator_leverage,
            ));
            let margin_coordinator =
                Amount::from_sat(position.coordinator_margin as u64) + extra_margin_coordinator;

            let original_accumulated_order_matching_fees = position.order_matching_fees;

            // The coordinator will not use the accrued fees to increase the margin.
            let effective_coordinator_collateral_reserve = original_coordinator_collateral_reserve
                .checked_sub(original_accumulated_order_matching_fees)
                .with_context(|| {
                    format!(
                        "This should not happen, but alas: \
                         original_accumulated_order_matching_fees ({}) > \
                         original_coordinator_collateral_reserve ({}) ",
                        original_accumulated_order_matching_fees,
                        original_coordinator_collateral_reserve
                    )
                })?;

            let coordinator_collateral_reserve = effective_coordinator_collateral_reserve
                .checked_sub(extra_margin_coordinator)
                .with_context(|| {
                    format!(
                        "Coordinator does not have enough funds to cover resize margin: \
                         extra_margin_coordinator ({}) > \
                         effective_coordinator_collateral_reserve ({})",
                        extra_margin_coordinator, effective_coordinator_collateral_reserve
                    )
                })?;

            let collateral_reserve_coordinator = coordinator_collateral_reserve
                + original_accumulated_order_matching_fees
                + order_matching_fee;

            let extra_margin_trader = Amount::from_sat(calculate_margin(
                order_execution_price,
                order_contracts.to_f32().expect("to fit"),
                position.trader_leverage,
            ));
            let margin_trader =
                Amount::from_sat(position.trader_margin as u64) + extra_margin_trader;

            let collateral_reserve_trader = original_trader_collateral_reserve
                .checked_sub(order_matching_fee)
                .and_then(|collateral| collateral.checked_sub(extra_margin_trader))
                .with_context(|| {
                    format!(
                        "Coordinator does not have enough funds to cover resize margin: \
                         extra_margin_trader ({}) + order_matching_fee ({}) > \
                         original_trader_collateral_reserve ({})",
                        extra_margin_trader, order_matching_fee, original_trader_collateral_reserve
                    )
                })?;

            let starting_contracts = Decimal::from_f32(position.quantity).expect("to fit");

            let total_contracts = starting_contracts + order_contracts;

            let starting_average_execution_price =
                Decimal::from_f32(position.average_entry_price).expect("to fit");

            let average_execution_price = (starting_contracts + order_contracts)
                / (starting_contracts / starting_average_execution_price
                    + order_contracts / order_execution_price);

            let coordinator_direction = position.trader_direction.opposite();

            let realized_pnl = None;

            let coordinator_liquidation_price = liquidation_price(
                average_execution_price,
                Decimal::try_from(position.coordinator_leverage).expect("to fit"),
                position.trader_direction.opposite(),
                maintenance_margin_rate,
            );

            let trader_liquidation_price = liquidation_price(
                average_execution_price,
                Decimal::try_from(position.trader_leverage).expect("to fit"),
                position.trader_direction,
                maintenance_margin_rate,
            );

            ResizedPosition {
                contracts: total_contracts,
                coordinator_direction,
                average_execution_price,
                coordinator_liquidation_price,
                trader_liquidation_price,
                margin_coordinator,
                margin_trader,
                collateral_reserve_coordinator,
                collateral_reserve_trader,
                realized_pnl,
            }
        }
        ResizeAction::Decrease {
            contracts: order_contracts,
            average_execution_price: order_average_execution_price,
        } => {
            let position_contracts = Decimal::try_from(position.quantity).expect("to fit");
            let total_contracts = position_contracts - order_contracts;

            let coordinator_direction = position.trader_direction.opposite();

            let position_average_execution_price =
                Decimal::try_from(position.average_entry_price).expect("to fit");
            let trader_liquidation_price =
                Decimal::try_from(position.trader_liquidation_price).expect("to fit");
            let coordinator_liquidation_price =
                Decimal::try_from(position.coordinator_liquidation_price).expect("to fit");

            let margin_coordinator = Amount::from_sat(calculate_margin(
                position_average_execution_price,
                total_contracts.to_f32().expect("to fit"),
                position.coordinator_leverage,
            ));

            let margin_trader = Amount::from_sat(calculate_margin(
                position_average_execution_price,
                total_contracts.to_f32().expect("to fit"),
                position.trader_leverage,
            ));

            let (original_margin_long, original_margin_short) = match position.trader_direction {
                Direction::Long => (
                    position.trader_margin as u64,
                    position.coordinator_margin as u64,
                ),
                Direction::Short => (
                    position.coordinator_margin as u64,
                    position.trader_margin as u64,
                ),
            };

            // The PNL is capped by the margin, so the coordinator should never end up eating into
            // the accrued order matching fees to pay the trader.
            let realized_pnl_trader = calculate_pnl(
                position_average_execution_price,
                order_average_execution_price,
                order_contracts.to_f32().expect("to fit"),
                position.trader_direction,
                original_margin_long,
                original_margin_short,
            )?;
            let realized_pnl_trader = SignedAmount::from_sat(realized_pnl_trader);

            let collateral_reserve_coordinator = {
                let margin_coordinator_before =
                    Amount::from_sat(position.coordinator_margin as u64);
                let margin_decrease = margin_coordinator_before
                    .checked_sub(margin_coordinator)
                    .with_context(|| {
                        format!(
                            "Nonsense margin change for position decrease: \
                             before ({margin_coordinator_before}) < after ({margin_coordinator})"
                        )
                    })?;

                let reserve = original_coordinator_collateral_reserve
                    .to_signed()
                    .expect("to fit")
                    + margin_decrease.to_signed().expect("to fit")
                    - realized_pnl_trader;

                let reserve = reserve.to_unsigned().with_context(|| {
                    format!(
                        "Position decrease leaves coordinator with negative reserve: \
                         original_collateral_reserve: {original_coordinator_collateral_reserve}, \
                         margin_decrease: {margin_decrease}, \
                         realized_pnl_trader: {realized_pnl_trader}"
                    )
                })?;

                reserve + order_matching_fee
            };

            let collateral_reserve_trader = {
                let margin_trader_before = Amount::from_sat(position.trader_margin as u64);
                let margin_decrease = margin_trader_before
                    .checked_sub(margin_trader)
                    .with_context(|| {
                        format!(
                            "Nonsense margin change for position decrease: \
                             before ({margin_trader_before}) < after ({margin_trader})"
                        )
                    })?;

                let reserve = original_trader_collateral_reserve
                    .to_signed()
                    .expect("to fit")
                    + margin_decrease.to_signed().expect("to fit")
                    + realized_pnl_trader
                    - order_matching_fee.to_signed().expect("to fit");

                reserve.to_unsigned().with_context(|| {
                    format!(
                        "Position decrease leaves trader with negative reserve: \
                         original_collateral_reserve: {original_trader_collateral_reserve}, \
                         margin_decrease: {margin_decrease}, \
                         realized_pnl_trader: {realized_pnl_trader}, \
                         order_matching_fee: {order_matching_fee}"
                    )
                })?
            };

            ResizedPosition {
                contracts: total_contracts,
                coordinator_direction,
                average_execution_price: position_average_execution_price,
                coordinator_liquidation_price,
                trader_liquidation_price,
                margin_coordinator,
                margin_trader,
                collateral_reserve_coordinator,
                collateral_reserve_trader,
                realized_pnl: Some(realized_pnl_trader),
            }
        }
        ResizeAction::ChangeDirection {
            contracts_new_direction,
            average_execution_price: order_average_execution_price,
        } => {
            let trader_direction = position.trader_direction.opposite();
            let coordinator_direction = trader_direction.opposite();

            let trader_liquidation_price = liquidation_price(
                order_average_execution_price,
                Decimal::try_from(position.trader_leverage).expect("to fit"),
                trader_direction,
                maintenance_margin_rate,
            );

            let coordinator_liquidation_price = liquidation_price(
                order_average_execution_price,
                Decimal::try_from(position.coordinator_leverage).expect("to fit"),
                trader_direction.opposite(),
                maintenance_margin_rate,
            );

            let new_margin_coordinator = Amount::from_sat(calculate_margin(
                order_average_execution_price,
                contracts_new_direction.to_f32().expect("to fit"),
                position.coordinator_leverage,
            ));

            let new_margin_trader = Amount::from_sat(calculate_margin(
                order_average_execution_price,
                contracts_new_direction.to_f32().expect("to fit"),
                position.trader_leverage,
            ));

            let position_average_execution_price =
                Decimal::try_from(position.average_entry_price).expect("to fit");
            let (original_margin_long, original_margin_short) = match position.trader_direction {
                Direction::Long => (
                    position.trader_margin as u64,
                    position.coordinator_margin as u64,
                ),
                Direction::Short => (
                    position.coordinator_margin as u64,
                    position.trader_margin as u64,
                ),
            };

            // The PNL is capped by the margin, so the coordinator should never end up eating into
            // the accrued order matching fees to pay the trader.
            let realized_pnl_trader = calculate_pnl(
                position_average_execution_price,
                order_average_execution_price,
                position.quantity,
                position.trader_direction,
                original_margin_long,
                original_margin_short,
            )?;
            let realized_pnl_trader = SignedAmount::from_sat(realized_pnl_trader);

            let collateral_reserve_trader = {
                let original_reserve = original_trader_collateral_reserve
                    .to_signed()
                    .expect("to fit");

                let closed_margin = SignedAmount::from_sat(calculate_margin(
                    position_average_execution_price,
                    position.quantity,
                    position.trader_leverage,
                ) as i64);

                let new_margin = new_margin_trader.to_signed().expect("to fit");

                let order_matching_fee = order_matching_fee.to_signed().expect("to fit");

                let reserve = original_reserve + closed_margin - new_margin + realized_pnl_trader
                    - order_matching_fee;

                reserve.to_unsigned().with_context(|| {
                    format!(
                        "Position direction change leaves trader with negative reserve: \
                         original_collateral_reserve: {original_trader_collateral_reserve}, \
                         closed_margin: {closed_margin}, \
                         new_margin: {new_margin_trader}, \
                         realized_pnl_trader: {realized_pnl_trader}, \
                         order_matching_fee {order_matching_fee}"
                    )
                })?
            };

            let collateral_reserve_coordinator = {
                let total_channel_collateral =
                    Amount::from_sat((position.trader_margin + position.coordinator_margin) as u64)
                        + original_coordinator_collateral_reserve
                        + original_trader_collateral_reserve;

                total_channel_collateral
                    .checked_sub(
                        collateral_reserve_trader + new_margin_trader + new_margin_coordinator,
                    )
                    .with_context(|| {
                        format!(
                            "Computed negative coordinator collateral reserve: \
                             total_channel_collateral ({total_channel_collateral}) < \
                             collateral_reserve_trader ({collateral_reserve_trader}) + \
                             new_margin_trader ({new_margin_trader}) + \
                             new_margin_coordinator ({new_margin_coordinator})"
                        )
                    })?
            };

            ResizedPosition {
                contracts: contracts_new_direction,
                coordinator_direction,
                average_execution_price: order_average_execution_price,
                coordinator_liquidation_price,
                trader_liquidation_price,
                margin_coordinator: new_margin_coordinator,
                margin_trader: new_margin_trader,
                collateral_reserve_coordinator,
                collateral_reserve_trader,
                realized_pnl: Some(realized_pnl_trader),
            }
        }
    };

    Ok(resized_position)
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

fn liquidation_price(
    price: Decimal,
    coordinator_leverage: Decimal,
    direction: Direction,
    maintenance_margin: Decimal,
) -> Decimal {
    match direction {
        Direction::Long => {
            calculate_long_liquidation_price(coordinator_leverage, price, maintenance_margin)
        }
        Direction::Short => {
            calculate_short_liquidation_price(coordinator_leverage, price, maintenance_margin)
        }
    }
}

pub fn coordinator_leverage_for_trade(_counterparty_peer_id: &PublicKey) -> Result<f32> {
    // TODO(bonomat): we will need to configure the leverage on the coordinator differently now
    // let channel_details = self.get_counterparty_channel(*counterparty_peer_id)?;
    // let user_channel_id = Uuid::from_u128(channel_details.user_channel_id).to_string();
    // let channel = db::channels::get(&user_channel_id, &mut conn)?.with_context(|| {
    //     format!("Couldn't find shadow channel with user channel ID {user_channel_id}",)
    // })?;
    // let leverage_coordinator = match channel.liquidity_option_id {
    //     Some(liquidity_option_id) => {
    //         let liquidity_option = db::liquidity_options::get(&mut conn,
    // liquidity_option_id)?;         liquidity_option.coordinator_leverage
    //     }
    //     None => 1.0,
    // };

    let leverage_coordinator = 2.0;

    Ok(leverage_coordinator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use trade::ContractSymbol;

    #[test]
    fn apply_resize() {
        check(
            100.0,
            Direction::Long,
            dec!(27_491.0),
            2.0,
            2.0,
            Amount::from_sat(1_091),
            ResizeAction::Increase {
                contracts: dec!(100),
                average_execution_price: dec!(20_000),
            },
            Amount::from_sat(2_000),
            Amount::from_sat(319_213),
            Amount::from_sat(319_213),
            dec!(0.1),
        );

        check(
            500.0,
            Direction::Long,
            dec!(27_336.5),
            2.0,
            2.0,
            Amount::from_sat(5_487),
            ResizeAction::Increase {
                contracts: dec!(100),
                average_execution_price: dec!(28_251),
            },
            Amount::from_sat(1_062),
            Amount::from_sat(2_090_959),
            Amount::from_sat(5_085_472),
            dec!(0.1),
        );

        check(
            100.0,
            Direction::Long,
            dec!(27_491.0),
            2.0,
            2.0,
            Amount::from_sat(1_091),
            ResizeAction::Decrease {
                contracts: dec!(50),
                average_execution_price: dec!(20_000),
            },
            Amount::from_sat(2_000),
            Amount::from_sat(319_213),
            Amount::from_sat(319_213),
            dec!(0.1),
        );

        check(
            100.0,
            Direction::Long,
            dec!(27_491.0),
            2.0,
            2.0,
            Amount::from_sat(1_091),
            ResizeAction::ChangeDirection {
                contracts_new_direction: dec!(100),
                average_execution_price: dec!(20_000),
            },
            Amount::from_sat(2_000),
            Amount::from_sat(319_213),
            Amount::from_sat(319_213),
            dec!(0.1),
        );

        check(
            50.0,
            Direction::Long,
            dec!(27_491.0),
            2.0,
            2.0,
            Amount::from_sat(1_091),
            ResizeAction::ChangeDirection {
                contracts_new_direction: dec!(100),
                average_execution_price: dec!(20_000),
            },
            Amount::from_sat(2_000),
            Amount::from_sat(319_213),
            Amount::from_sat(319_213),
            dec!(0.1),
        );
    }

    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    fn check(
        quantity: f32,
        trader_direction: Direction,
        average_entry_price: Decimal,
        coordinator_leverage: f32,
        trader_leverage: f32,
        accumulated_order_matching_fees: Amount,
        resize_action: ResizeAction,
        order_matching_fee: Amount,
        original_coordinator_collateral_reserve: Amount,
        original_trader_collateral_reserve: Amount,
        maintenance_margin: Decimal,
    ) {
        let coordinator_liquidation_price = liquidation_price(
            average_entry_price,
            Decimal::try_from(coordinator_leverage).unwrap(),
            trader_direction.opposite(),
            maintenance_margin,
        );
        let trader_liquidation_price = liquidation_price(
            average_entry_price,
            Decimal::try_from(trader_leverage).unwrap(),
            trader_direction,
            maintenance_margin,
        );

        let coordinator_margin =
            calculate_margin(average_entry_price, quantity, coordinator_leverage);
        let trader_margin = calculate_margin(average_entry_price, quantity, trader_leverage);

        let resized_position = apply_resize_to_position(
            resize_action,
            &Position {
                id: 1,
                trader: PublicKey::from_str(
                    "02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655",
                )
                .unwrap(),
                contract_symbol: ContractSymbol::BtcUsd,
                quantity,
                trader_direction,
                average_entry_price: average_entry_price.to_f32().unwrap(),
                closing_price: None,
                trader_realized_pnl_sat: None,
                coordinator_liquidation_price: coordinator_liquidation_price.to_f32().unwrap(),
                trader_liquidation_price: trader_liquidation_price.to_f32().unwrap(),
                trader_margin: trader_margin as i64,
                coordinator_margin: coordinator_margin as i64,
                trader_leverage,
                coordinator_leverage,
                position_state: PositionState::Open,
                order_matching_fees: accumulated_order_matching_fees,
                creation_timestamp: OffsetDateTime::now_utc(),
                expiry_timestamp: OffsetDateTime::now_utc(),
                update_timestamp: OffsetDateTime::now_utc(),
                temporary_contract_id: None,
                stable: false,
            },
            original_coordinator_collateral_reserve,
            original_trader_collateral_reserve,
            order_matching_fee,
            maintenance_margin,
        )
        .unwrap();

        insta::with_settings!({
            snapshot_suffix => format!(
                "{trader_direction:?}-{quantity}-{}",
                resize_action.name()
            )
        }, {
            assert_debug_snapshot!(resized_position);
        });
    }

    impl ResizeAction {
        fn name(&self) -> &str {
            match self {
                ResizeAction::Increase { .. } => "increase",
                ResizeAction::Decrease { .. } => "decrease",
                ResizeAction::ChangeDirection { .. } => "change-direction",
            }
        }
    }
}
