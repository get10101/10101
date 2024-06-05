use crate::db;
use crate::funding_fee::insert_protocol_funding_fee_event;
use crate::funding_fee::mark_funding_fee_event_as_paid;
use crate::position::models::PositionState;
use crate::trade::models::NewTrade;
use crate::trade::websocket::InternalPositionUpdateMessage;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use bitcoin::SignedAmount;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::result::Error::RollbackTransaction;
use diesel::Connection;
use diesel::PgConnection;
use diesel::QueryResult;
use dlc_manager::ContractId;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use tokio::sync::broadcast::Sender;
use xxi_node::cfd::calculate_pnl;
use xxi_node::commons;
use xxi_node::commons::Direction;
use xxi_node::node::rust_dlc_manager::DlcChannelId;
use xxi_node::node::ProtocolId;

pub struct DlcProtocol {
    pub id: ProtocolId,
    pub previous_id: Option<ProtocolId>,
    pub timestamp: OffsetDateTime,
    pub channel_id: DlcChannelId,
    pub contract_id: Option<ContractId>,
    pub trader: PublicKey,
    pub protocol_state: DlcProtocolState,
    pub protocol_type: DlcProtocolType,
}

#[derive(Clone, Copy, Debug)]
pub struct TradeParams {
    pub protocol_id: ProtocolId,
    pub trader: PublicKey,
    pub quantity: f32,
    pub leverage: f32,
    pub average_price: f32,
    pub direction: Direction,
    pub matching_fee: Amount,
    pub trader_pnl: Option<SignedAmount>,
}

impl TradeParams {
    fn new(
        trade_params: &commons::TradeParams,
        protocol_id: ProtocolId,
        trader_pnl: Option<SignedAmount>,
    ) -> Self {
        Self {
            protocol_id,
            trader: trade_params.pubkey,
            quantity: trade_params.quantity,
            leverage: trade_params.leverage,
            average_price: trade_params
                .average_execution_price()
                .to_f32()
                .expect("to fit"),
            direction: trade_params.direction,
            matching_fee: trade_params.order_matching_fee(),
            trader_pnl,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RolloverParams {
    pub protocol_id: ProtocolId,
    pub trader_pubkey: PublicKey,
    pub margin_coordinator: Amount,
    pub margin_trader: Amount,
    pub leverage_coordinator: Decimal,
    pub leverage_trader: Decimal,
    pub liquidation_price_coordinator: Decimal,
    pub liquidation_price_trader: Decimal,
    pub expiry_timestamp: OffsetDateTime,
}

pub enum DlcProtocolState {
    Pending,
    Success,
    Failed,
}

#[derive(Clone, Debug)]
pub enum DlcProtocolType {
    /// Opening a channel also opens a position.
    OpenChannel {
        trade_params: TradeParams,
    },
    OpenPosition {
        trade_params: TradeParams,
    },
    ResizePosition {
        trade_params: TradeParams,
    },
    Rollover {
        rollover_params: RolloverParams,
    },
    Settle {
        trade_params: TradeParams,
    },
    Close {
        trader: PublicKey,
    },
    ForceClose {
        trader: PublicKey,
    },
}

pub struct DlcProtocolExecutor {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl DlcProtocolExecutor {
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        DlcProtocolExecutor { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_open_channel_protocol(
        &self,
        protocol_id: ProtocolId,
        temporary_contract_id: &ContractId,
        temporary_channel_id: &DlcChannelId,
        trade_params: &commons::TradeParams,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        conn.transaction(|conn| {
            let trader_pubkey = trade_params.pubkey;

            db::dlc_protocols::create(
                conn,
                protocol_id,
                None,
                Some(temporary_contract_id),
                temporary_channel_id,
                db::dlc_protocols::DlcProtocolType::OpenChannel,
                &trader_pubkey,
            )?;

            db::trade_params::insert(conn, &TradeParams::new(trade_params, protocol_id, None))?;

            diesel::result::QueryResult::Ok(())
        })?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_open_position_protocol(
        &self,
        protocol_id: ProtocolId,
        previous_protocol_id: Option<ProtocolId>,
        temporary_contract_id: &ContractId,
        channel_id: &DlcChannelId,
        trade_params: &commons::TradeParams,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        conn.transaction(|conn| {
            let trader_pubkey = trade_params.pubkey;

            db::dlc_protocols::create(
                conn,
                protocol_id,
                previous_protocol_id,
                Some(temporary_contract_id),
                channel_id,
                db::dlc_protocols::DlcProtocolType::OpenPosition,
                &trader_pubkey,
            )?;

            db::trade_params::insert(conn, &TradeParams::new(trade_params, protocol_id, None))?;

            diesel::result::QueryResult::Ok(())
        })?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_resize_protocol(
        &self,
        protocol_id: ProtocolId,
        previous_protocol_id: Option<ProtocolId>,
        temporary_contract_id: Option<&ContractId>,
        channel_id: &DlcChannelId,
        trade_params: &commons::TradeParams,
        realized_pnl: Option<SignedAmount>,
        funding_fee_event_ids: Vec<i32>,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        conn.transaction(|conn| {
            let trader_pubkey = trade_params.pubkey;

            db::dlc_protocols::create(
                conn,
                protocol_id,
                previous_protocol_id,
                temporary_contract_id,
                channel_id,
                db::dlc_protocols::DlcProtocolType::ResizePosition,
                &trader_pubkey,
            )?;

            insert_protocol_funding_fee_event(conn, protocol_id, &funding_fee_event_ids)?;

            db::trade_params::insert(
                conn,
                &TradeParams::new(trade_params, protocol_id, realized_pnl),
            )?;

            diesel::result::QueryResult::Ok(())
        })?;

        Ok(())
    }

    pub fn start_settle_protocol(
        &self,
        protocol_id: ProtocolId,
        previous_protocol_id: Option<ProtocolId>,
        contract_id: &ContractId,
        channel_id: &DlcChannelId,
        trade_params: &commons::TradeParams,
        funding_fee_event_ids: Vec<i32>,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        conn.transaction(|conn| {
            let trader_pubkey = trade_params.pubkey;

            db::dlc_protocols::create(
                conn,
                protocol_id,
                previous_protocol_id,
                Some(contract_id),
                channel_id,
                db::dlc_protocols::DlcProtocolType::Settle,
                &trader_pubkey,
            )?;

            insert_protocol_funding_fee_event(conn, protocol_id, &funding_fee_event_ids)?;

            db::trade_params::insert(conn, &TradeParams::new(trade_params, protocol_id, None))?;

            diesel::result::QueryResult::Ok(())
        })?;

        Ok(())
    }

    /// Persist a new rollover protocol and update technical tables in a single transaction.
    pub fn start_rollover(
        &self,
        protocol_id: ProtocolId,
        previous_protocol_id: Option<ProtocolId>,
        temporary_contract_id: &ContractId,
        channel_id: &DlcChannelId,
        rollover_params: RolloverParams,
        funding_fee_event_ids: Vec<i32>,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        conn.transaction(|conn| {
            let trader_pubkey = rollover_params.trader_pubkey;

            db::dlc_protocols::create(
                conn,
                protocol_id,
                previous_protocol_id,
                Some(temporary_contract_id),
                channel_id,
                db::dlc_protocols::DlcProtocolType::Rollover,
                &trader_pubkey,
            )?;

            insert_protocol_funding_fee_event(conn, protocol_id, &funding_fee_event_ids)?;

            db::rollover_params::insert(conn, &rollover_params)?;

            diesel::result::QueryResult::Ok(())
        })?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_close_channel_protocol(
        &self,
        protocol_id: ProtocolId,
        previous_protocol_id: Option<ProtocolId>,
        channel_id: &DlcChannelId,
        trader_id: &PublicKey,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        db::dlc_protocols::create(
            &mut conn,
            protocol_id,
            previous_protocol_id,
            None,
            channel_id,
            db::dlc_protocols::DlcProtocolType::Close,
            trader_id,
        )?;

        Ok(())
    }

    pub fn fail_dlc_protocol(&self, protocol_id: ProtocolId) -> Result<()> {
        let mut conn = self.pool.get()?;
        db::dlc_protocols::set_dlc_protocol_state_to_failed(&mut conn, protocol_id)?;

        Ok(())
    }

    /// Update the state of the database and the position feed based on the completion of a DLC
    /// protocol.
    pub fn finish_dlc_protocol(
        &self,
        protocol_id: ProtocolId,
        trader_id: &PublicKey,
        contract_id: Option<ContractId>,
        channel_id: &DlcChannelId,
        tx_position_feed: Sender<InternalPositionUpdateMessage>,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        let dlc_protocol = db::dlc_protocols::get_dlc_protocol(&mut conn, protocol_id)?;
        conn.transaction(|conn| {
            match &dlc_protocol.protocol_type {
                DlcProtocolType::OpenChannel { trade_params }
                | DlcProtocolType::OpenPosition { trade_params } => {
                    let contract_id = contract_id
                        .context("missing contract id")
                        .map_err(|_| RollbackTransaction)?;
                    self.finish_open_position_dlc_protocol(
                        conn,
                        trade_params,
                        protocol_id,
                        &contract_id,
                        channel_id,
                    )
                }
                DlcProtocolType::ResizePosition { trade_params } => {
                    let contract_id = contract_id
                        .context("missing contract id")
                        .map_err(|_| RollbackTransaction)?;
                    self.finish_resize_position_dlc_protocol(
                        conn,
                        trade_params,
                        protocol_id,
                        &contract_id,
                        channel_id,
                    )
                }
                DlcProtocolType::Settle { trade_params } => {
                    let settled_contract = dlc_protocol.contract_id;
                    self.finish_settle_dlc_protocol(
                        conn,
                        trade_params,
                        protocol_id,
                        // If the contract got settled, we do not get a new contract id, hence we
                        // copy the contract id of the settled contract.
                        settled_contract.as_ref(),
                        channel_id,
                    )
                }
                DlcProtocolType::Rollover { rollover_params } => {
                    let contract_id = contract_id
                        .context("missing contract id")
                        .map_err(|_| RollbackTransaction)?;

                    self.finish_rollover_dlc_protocol(
                        conn,
                        trader_id,
                        protocol_id,
                        &contract_id,
                        channel_id,
                        rollover_params,
                    )
                }
                DlcProtocolType::Close { .. } => {
                    self.finish_close_channel_dlc_protocol(conn, trader_id, protocol_id, channel_id)
                }
                DlcProtocolType::ForceClose { .. } => {
                    debug_assert!(false, "Finishing unexpected dlc protocol types");
                    Ok(())
                }
            }
        })?;

        match &dlc_protocol.protocol_type {
            DlcProtocolType::OpenChannel { trade_params }
            | DlcProtocolType::OpenPosition { trade_params }
            | DlcProtocolType::ResizePosition { trade_params }
            | DlcProtocolType::Settle { trade_params } => {
                if let Err(e) = {
                    tx_position_feed.send(InternalPositionUpdateMessage::NewTrade {
                        quantity: if trade_params.direction == Direction::Short {
                            trade_params.quantity
                        } else {
                            // We want to reflect the quantity as seen by the coordinator
                            trade_params.quantity * -1.0
                        },
                        average_entry_price: trade_params.average_price,
                    })
                } {
                    tracing::error!("Could not notify channel about finished trade {e:#}");
                }
            }
            _ => {
                // A trade only happens in `OpenChannel`, `OpenPosition`, `ResizePosition` and
                // `Settle`.
            }
        }

        Ok(())
    }

    /// Complete the settle DLC protocol as successful and update the 10101 metadata accordingly in
    /// a single database transaction.
    ///
    /// - Set settle DLC protocol to success.
    ///
    /// - Calculate the PNL and update the `[PositionState::Closing`] to `[PositionState::Closed`].
    ///
    /// - Create and insert new trade.
    ///
    /// - Mark relevant funding fee events as paid.
    fn finish_settle_dlc_protocol(
        &self,
        conn: &mut PgConnection,
        trade_params: &TradeParams,
        protocol_id: ProtocolId,
        settled_contract: Option<&ContractId>,
        channel_id: &DlcChannelId,
    ) -> QueryResult<()> {
        db::dlc_protocols::set_dlc_protocol_state_to_success(
            conn,
            protocol_id,
            settled_contract,
            channel_id,
        )?;

        // TODO(holzeis): We are still updating the position based on the position state. This
        // will change once we only have a single position per user and representing
        // the position only as view on multiple trades.
        let position = match db::positions::Position::get_position_by_trader(
            conn,
            trade_params.trader,
            vec![
                // The price doesn't matter here.
                PositionState::Closing { closing_price: 0.0 },
            ],
        )? {
            Some(position) => position,
            None => {
                tracing::error!("No position in state Closing found.");
                return Err(RollbackTransaction);
            }
        };

        tracing::debug!(
            ?position,
            trader_id = %trade_params.trader,
            "Finalize closing position",
        );

        let trader_realized_pnl_sat = {
            let trader_position_direction = position.trader_direction;

            let (initial_margin_long, initial_margin_short) = match trader_position_direction {
                Direction::Long => (position.trader_margin, position.coordinator_margin),
                Direction::Short => (position.coordinator_margin, position.trader_margin),
            };

            match calculate_pnl(
                Decimal::from_f32(position.average_entry_price).expect("to fit into decimal"),
                Decimal::from_f32(trade_params.average_price).expect("to fit into decimal"),
                trade_params.quantity,
                trader_position_direction,
                initial_margin_long.to_sat(),
                initial_margin_short.to_sat(),
            ) {
                Ok(pnl) => pnl,
                Err(e) => {
                    tracing::error!("Failed to calculate pnl. Error: {e:#}");
                    return Err(RollbackTransaction);
                }
            }
        };

        let closing_price =
            Decimal::try_from(trade_params.average_price).expect("to fit into decimal");

        db::positions::Position::set_position_to_closed_with_pnl(
            conn,
            position.id,
            trader_realized_pnl_sat,
            closing_price,
        )?;

        let order_matching_fee = trade_params.matching_fee;

        let new_trade = NewTrade {
            position_id: position.id,
            contract_symbol: position.contract_symbol,
            trader_pubkey: trade_params.trader,
            quantity: trade_params.quantity,
            trader_leverage: trade_params.leverage,
            trader_direction: trade_params.direction,
            average_price: trade_params.average_price,
            order_matching_fee,
            trader_realized_pnl_sat: Some(trader_realized_pnl_sat),
        };

        db::trades::insert(conn, new_trade)?;

        mark_funding_fee_event_as_paid(conn, protocol_id)?;

        Ok(())
    }

    /// Complete a DLC protocol that opens a position, by updating several database tables in a
    /// single transaction.
    ///
    /// Specifically, we:
    ///
    /// - Set DLC protocol to success.
    /// - Update the position state to [`PositionState::Open`].
    /// - Create and insert the new trade.
    fn finish_open_position_dlc_protocol(
        &self,
        conn: &mut PgConnection,
        trade_params: &TradeParams,
        protocol_id: ProtocolId,
        contract_id: &ContractId,
        channel_id: &DlcChannelId,
    ) -> QueryResult<()> {
        db::dlc_protocols::set_dlc_protocol_state_to_success(
            conn,
            protocol_id,
            Some(contract_id),
            channel_id,
        )?;

        // TODO(holzeis): We are still updating the position based on the position state. This
        // will change once we only have a single position per user and representing
        // the position only as view on multiple trades.
        let position = db::positions::Position::update_position_state(
            conn,
            trade_params.trader.to_string(),
            vec![PositionState::Proposed],
            PositionState::Open,
        )?;

        let order_matching_fee = trade_params.matching_fee;

        let new_trade = NewTrade {
            position_id: position.id,
            contract_symbol: position.contract_symbol,
            trader_pubkey: trade_params.trader,
            quantity: trade_params.quantity,
            trader_leverage: trade_params.leverage,
            trader_direction: trade_params.direction,
            average_price: trade_params.average_price,
            order_matching_fee,
            trader_realized_pnl_sat: None,
        };

        db::trades::insert(conn, new_trade)?;

        Ok(())
    }

    /// Complete a DLC protocol that resizes a position, by updating several database tables in a
    /// single transaction.
    fn finish_resize_position_dlc_protocol(
        &self,
        conn: &mut PgConnection,
        trade_params: &TradeParams,
        protocol_id: ProtocolId,
        contract_id: &ContractId,
        channel_id: &DlcChannelId,
    ) -> QueryResult<()> {
        db::dlc_protocols::set_dlc_protocol_state_to_success(
            conn,
            protocol_id,
            Some(contract_id),
            channel_id,
        )?;

        // TODO(holzeis): We are still updating the position based on the position state. This
        // will change once we only have a single position per user and representing
        // the position only as view on multiple trades.
        let position = db::positions::Position::update_position_state(
            conn,
            trade_params.trader.to_string(),
            vec![PositionState::Resizing],
            PositionState::Open,
        )?;

        let order_matching_fee = trade_params.matching_fee;

        let new_trade = NewTrade {
            position_id: position.id,
            contract_symbol: position.contract_symbol,
            trader_pubkey: trade_params.trader,
            quantity: trade_params.quantity,
            trader_leverage: trade_params.leverage,
            trader_direction: trade_params.direction,
            average_price: trade_params.average_price,
            order_matching_fee,
            trader_realized_pnl_sat: trade_params.trader_pnl.map(|pnl| pnl.to_sat()),
        };

        db::trades::insert(conn, new_trade)?;

        mark_funding_fee_event_as_paid(conn, protocol_id)?;

        Ok(())
    }

    /// Complete the rollover DLC protocol as successful and update the 10101 metadata accordingly,
    /// in a single database transaction.
    fn finish_rollover_dlc_protocol(
        &self,
        conn: &mut PgConnection,
        trader: &PublicKey,
        protocol_id: ProtocolId,
        contract_id: &ContractId,
        channel_id: &DlcChannelId,
        rollover_params: &RolloverParams,
    ) -> QueryResult<()> {
        tracing::debug!(%trader, %protocol_id, "Finalizing rollover");
        db::dlc_protocols::set_dlc_protocol_state_to_success(
            conn,
            protocol_id,
            Some(contract_id),
            channel_id,
        )?;

        db::positions::Position::finish_rollover_protocol(
            conn,
            trader.to_string(),
            *contract_id,
            rollover_params.leverage_coordinator,
            rollover_params.margin_coordinator,
            rollover_params.liquidation_price_coordinator,
            rollover_params.leverage_trader,
            rollover_params.margin_trader,
            rollover_params.liquidation_price_trader,
        )?;

        mark_funding_fee_event_as_paid(conn, protocol_id)?;

        Ok(())
    }

    /// Completes the collab close dlc protocol as successful
    fn finish_close_channel_dlc_protocol(
        &self,
        conn: &mut PgConnection,
        trader: &PublicKey,
        protocol_id: ProtocolId,
        channel_id: &DlcChannelId,
    ) -> QueryResult<()> {
        tracing::debug!(%trader, %protocol_id, "Finalizing channel close");
        db::dlc_protocols::set_dlc_protocol_state_to_success(conn, protocol_id, None, channel_id)
    }
}

#[cfg(test)]
mod test {
    use crate::dlc_protocol::ProtocolId;
    use dlc_manager::ReferenceId;

    #[test]
    fn test_protocol_id_roundtrip() {
        let protocol_id_0 = ProtocolId::new();

        let reference_id = ReferenceId::from(protocol_id_0);

        let protocol_id_1 = ProtocolId::try_from(reference_id).unwrap();

        assert_eq!(protocol_id_0, protocol_id_1)
    }
}
