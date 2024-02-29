use crate::db;
use crate::position::models::PositionState;
use crate::trade::models::NewTrade;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::result::Error::RollbackTransaction;
use diesel::Connection;
use diesel::PgConnection;
use dlc_manager::ContractId;
use dlc_manager::ReferenceId;
use ln_dlc_node::node::rust_dlc_manager::DlcChannelId;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::from_utf8;
use time::OffsetDateTime;
use trade::cfd::calculate_margin;
use trade::cfd::calculate_pnl;
use trade::Direction;
use uuid::Uuid;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ProtocolId(Uuid);

impl ProtocolId {
    pub fn new() -> Self {
        ProtocolId(Uuid::new_v4())
    }

    pub fn to_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ProtocolId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ProtocolId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.to_string().fmt(f)
    }
}

impl From<ProtocolId> for ReferenceId {
    fn from(value: ProtocolId) -> Self {
        let uuid = value.to_uuid();

        // 16 bytes.
        let uuid_bytes = uuid.as_bytes();

        // 32-digit hex string.
        let hex = hex::encode(uuid_bytes);

        // Derived `ReferenceId`: 32-bytes.
        let hex_bytes = hex.as_bytes();

        let mut array = [0u8; 32];
        array.copy_from_slice(hex_bytes);

        array
    }
}

impl TryFrom<ReferenceId> for ProtocolId {
    type Error = anyhow::Error;

    fn try_from(value: ReferenceId) -> Result<Self> {
        // 32-digit hex string.
        let hex = from_utf8(&value)?;

        // 16 bytes.
        let uuid_bytes = hex::decode(hex)?;

        let uuid = Uuid::from_slice(&uuid_bytes)?;

        Ok(ProtocolId(uuid))
    }
}

impl From<Uuid> for ProtocolId {
    fn from(value: Uuid) -> Self {
        ProtocolId(value)
    }
}

impl From<ProtocolId> for Uuid {
    fn from(value: ProtocolId) -> Self {
        value.0
    }
}

pub struct DlcProtocol {
    pub id: ProtocolId,
    pub timestamp: OffsetDateTime,
    pub channel_id: DlcChannelId,
    pub contract_id: ContractId,
    pub trader: PublicKey,
    pub protocol_state: DlcProtocolState,
}

pub struct TradeParams {
    pub protocol_id: ProtocolId,
    pub trader: PublicKey,
    pub quantity: f32,
    pub leverage: f32,
    pub average_price: f32,
    pub direction: Direction,
}

pub enum DlcProtocolState {
    Pending,
    Success,
    Failed,
}

pub struct DlcProtocolExecutor {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl DlcProtocolExecutor {
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        DlcProtocolExecutor { pool }
    }

    /// Starts a dlc protocol, by creating a new dlc protocol and temporarily stores
    /// the trade params.
    ///
    /// Returns a uniquely generated protocol id as [`dlc_manager::ReferenceId`]
    pub fn start_dlc_protocol(
        &self,
        protocol_id: ProtocolId,
        previous_protocol_id: Option<ProtocolId>,
        contract_id: &ContractId,
        channel_id: &DlcChannelId,
        trade_params: &commons::TradeParams,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        conn.transaction(|conn| {
            db::dlc_protocols::create(
                conn,
                protocol_id,
                previous_protocol_id,
                contract_id,
                channel_id,
                &trade_params.pubkey,
            )?;
            db::trade_params::insert(conn, protocol_id, trade_params)?;

            diesel::result::QueryResult::Ok(())
        })?;

        Ok(())
    }

    pub fn fail_dlc_protocol(&self, protocol_id: ProtocolId) -> Result<()> {
        let mut conn = self.pool.get()?;
        db::dlc_protocols::set_dlc_protocol_state_to_failed(&mut conn, protocol_id)?;

        Ok(())
    }

    /// Completes the dlc protocol as successful and updates the 10101 meta data
    /// accordingly in a single database transaction.
    /// - Set dlc protocol to success
    /// - If not closing: Updates the `[PostionState::Proposed`] position state to
    ///   `[PostionState::Open]`
    /// - If closing: Calculates the pnl and sets the `[PositionState::Closing`] position state to
    ///   `[PositionState::Closed`]
    /// - Creates and inserts the new trade
    pub fn finish_dlc_protocol(
        &self,
        protocol_id: ProtocolId,
        closing: bool,
        contract_id: ContractId,
        channel_id: DlcChannelId,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;

        conn.transaction(|conn| {
            let trade_params: TradeParams = db::trade_params::get(conn, protocol_id)?;

            db::dlc_protocols::set_dlc_protocol_state_to_success(
                conn,
                protocol_id,
                contract_id,
                channel_id,
            )?;

            // TODO(holzeis): We are still updating the position based on the position state. This
            // will change once we only have a single position per user and representing
            // the position only as view on multiple trades.
            let position = match closing {
                false => db::positions::Position::update_proposed_position(
                    conn,
                    trade_params.trader.to_string(),
                    PositionState::Open,
                ),
                true => {
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

                    let pnl = {
                        let (initial_margin_long, initial_margin_short) =
                            match trade_params.direction {
                                Direction::Long => {
                                    (position.trader_margin, position.coordinator_margin)
                                }
                                Direction::Short => {
                                    (position.coordinator_margin, position.trader_margin)
                                }
                            };

                        match calculate_pnl(
                            Decimal::from_f32(position.average_entry_price)
                                .expect("to fit into decimal"),
                            Decimal::from_f32(trade_params.average_price)
                                .expect("to fit into decimal"),
                            trade_params.quantity,
                            trade_params.direction,
                            initial_margin_long as u64,
                            initial_margin_short as u64,
                        ) {
                            Ok(pnl) => pnl,
                            Err(e) => {
                                tracing::error!("Failed to calculate pnl. Error: {e:#}");
                                return Err(RollbackTransaction);
                            }
                        }
                    };

                    db::positions::Position::set_position_to_closed_with_pnl(conn, position.id, pnl)
                }
            }?;

            let coordinator_margin = calculate_margin(
                Decimal::try_from(trade_params.average_price).expect("to fit into decimal"),
                trade_params.quantity,
                crate::trade::coordinator_leverage_for_trade(&trade_params.trader)
                    .map_err(|_| RollbackTransaction)?,
            );

            // TODO(holzeis): Add optional pnl to trade.
            // Instead of tracking pnl on the position we want to track pnl on the trade. e.g. Long
            // -> Short or Short -> Long.
            let new_trade = NewTrade {
                position_id: position.id,
                contract_symbol: position.contract_symbol,
                trader_pubkey: trade_params.trader,
                quantity: trade_params.quantity,
                trader_leverage: trade_params.leverage,
                coordinator_margin: coordinator_margin as i64,
                trader_direction: trade_params.direction,
                average_price: trade_params.average_price,
                dlc_expiry_timestamp: None,
            };

            db::trades::insert(conn, new_trade)?;

            db::trade_params::delete(conn, protocol_id)
        })?;

        Ok(())
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
