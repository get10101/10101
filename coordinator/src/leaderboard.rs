use crate::db;
use crate::position::models::Position;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Serialize)]
pub struct LeaderBoard {
    pub(crate) entries: Vec<LeaderBoardEntry>,
}

#[derive(Serialize, Clone)]
pub struct LeaderBoardEntry {
    pub trader: PublicKey,
    pub nickname: String,
    pub pnl: Decimal,
    pub volume: Decimal,
    pub rank: usize,
}

#[derive(Debug, Deserialize)]
pub struct LeaderBoardQueryParams {
    pub(crate) top: Option<usize>,
    pub(crate) reverse: Option<bool>,
    pub(crate) category: Option<LeaderBoardCategory>,
    pub(crate) start: Option<String>,
    pub(crate) end: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub enum LeaderBoardCategory {
    Pnl,
    Volume,
}

/// Returns the traders
///
/// Optional arguments:
/// - `[top]` defines how many traders are returned, default to 5
/// - `[category]` can be `PnL` or `Volume`, default is `PnL`
/// - `[reverse]` will return the traders with the lowest values, default is `false`
pub(crate) fn generate_leader_board(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    top: usize,
    category: LeaderBoardCategory,
    reverse: bool,
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> Result<Vec<LeaderBoardEntry>> {
    let positions = load_positions(conn, start, end)?;

    let leader_board = sort_leader_board(top, category, reverse, positions);
    let leader_board = leader_board
        .into_iter()
        .map(|entry| {
            let nickname = db::user::get_user(conn, &entry.trader).unwrap_or_default();
            LeaderBoardEntry {
                nickname: nickname.and_then(|user| user.nickname).unwrap_or_default(),
                ..entry
            }
        })
        .collect();

    Ok(leader_board)
}

fn sort_leader_board(
    top: usize,
    category: LeaderBoardCategory,
    reverse: bool,
    positions: HashMap<PublicKey, Vec<Position>>,
) -> Vec<LeaderBoardEntry> {
    let mut leader_board = positions
        .into_iter()
        .map(|(trader, positions)| {
            LeaderBoardEntry {
                trader,
                nickname: "".to_string(),
                pnl: positions
                    .iter()
                    .map(|p| Decimal::from(p.trader_realized_pnl_sat.unwrap_or_default()))
                    .sum(),
                volume: positions
                    .iter()
                    .map(|p| Decimal::from_f32(p.quantity).expect("to fit into decimal"))
                    .sum(),
                // default all ranks are 0, this will be filled later
                rank: 0,
            }
        })
        .collect::<Vec<LeaderBoardEntry>>();

    leader_board.sort_by(|a, b| {
        if reverse {
            match category {
                LeaderBoardCategory::Pnl => a.pnl.cmp(&b.pnl),
                LeaderBoardCategory::Volume => a.volume.cmp(&b.volume),
            }
        } else {
            match category {
                LeaderBoardCategory::Pnl => b.pnl.cmp(&a.pnl),
                LeaderBoardCategory::Volume => b.volume.cmp(&a.volume),
            }
        }
    });

    let top_x = if top > leader_board.len() {
        leader_board.len()
    } else {
        top
    };

    let leader_board = &leader_board[0..top_x];
    let mut leader_board = leader_board.to_vec();
    for (index, entry) in leader_board.iter_mut().enumerate() {
        entry.rank = index + 1; // we want to start with the rank 1
    }
    leader_board
}

fn load_positions(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> Result<HashMap<PublicKey, Vec<Position>>> {
    let positions = db::positions::Position::get_all_closed_positions(conn)?;
    let positions = positions
        .into_iter()
        .filter(|pos| pos.creation_timestamp > start && pos.creation_timestamp < end)
        .collect::<Vec<_>>();

    let mut positions_by_trader = HashMap::new();

    for position in positions {
        let trader_id = position.trader;

        positions_by_trader
            .entry(trader_id)
            .or_insert(Vec::new())
            .push(position);
    }

    Ok(positions_by_trader)
}

#[cfg(test)]
pub mod tests {
    use crate::leaderboard::sort_leader_board;
    use crate::leaderboard::LeaderBoardCategory;
    use crate::position::models::Position;
    use crate::position::models::PositionState;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::Amount;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use std::str::FromStr;
    use time::OffsetDateTime;
    use trade::ContractSymbol;
    use trade::Direction;

    #[test]
    pub fn given_3_leaders_sort_by_pnl() {
        let trader_0 = leader_0();
        let trader_1 = leader_1();
        let trader_2 = leader_2();
        let pos_0 = create_dummy_position(trader_0, 100, 100.0);
        let pos_1 = create_dummy_position(trader_0, 100, 100.0);
        let pos_2 = create_dummy_position(trader_1, 0, 100.0);
        let pos_3 = create_dummy_position(trader_2, -100, 300.0);

        let positions: HashMap<PublicKey, Vec<Position>> = [
            (trader_0, vec![pos_0, pos_1]),
            (trader_1, vec![pos_2]),
            (trader_2, vec![pos_3]),
        ]
        .into();

        let leader_board = sort_leader_board(3, LeaderBoardCategory::Pnl, false, positions.clone());
        assert_eq!(leader_board.first().unwrap().pnl, dec!(200));
        assert_eq!(leader_board.first().unwrap().rank, 1);
        assert_eq!(leader_board.first().unwrap().trader, trader_0);

        assert_eq!(leader_board.get(1).unwrap().pnl, dec!(0));
        assert_eq!(leader_board.get(1).unwrap().rank, 2);
        assert_eq!(leader_board.get(1).unwrap().trader, trader_1);

        assert_eq!(leader_board.get(2).unwrap().pnl, dec!(-100));
        assert_eq!(leader_board.get(2).unwrap().rank, 3);
        assert_eq!(leader_board.get(2).unwrap().trader, trader_2);

        let leader_board = sort_leader_board(3, LeaderBoardCategory::Pnl, true, positions);
        assert_eq!(leader_board.first().unwrap().pnl, dec!(-100));
        assert_eq!(leader_board.first().unwrap().rank, 1);
        assert_eq!(leader_board.first().unwrap().trader, trader_2);

        assert_eq!(leader_board.get(1).unwrap().pnl, dec!(0));
        assert_eq!(leader_board.get(1).unwrap().rank, 2);
        assert_eq!(leader_board.get(1).unwrap().trader, trader_1);

        assert_eq!(leader_board.get(2).unwrap().pnl, dec!(200));
        assert_eq!(leader_board.get(2).unwrap().rank, 3);
        assert_eq!(leader_board.get(2).unwrap().trader, trader_0);
    }

    #[test]
    pub fn given_3_take_2_leaders_sort_by_volume() {
        let trader_0 = leader_0();
        let trader_1 = leader_1();
        let trader_2 = leader_2();
        let pos_0 = create_dummy_position(trader_0, 100, 100.0);
        let pos_1 = create_dummy_position(trader_0, 100, 100.0);
        let pos_2 = create_dummy_position(trader_1, 0, 100.0);
        let pos_3 = create_dummy_position(trader_2, -100, 300.0);

        let positions: HashMap<PublicKey, Vec<Position>> = [
            (trader_0, vec![pos_0, pos_1]),
            (trader_1, vec![pos_2]),
            (trader_2, vec![pos_3]),
        ]
        .into();

        let leader_board =
            sort_leader_board(2, LeaderBoardCategory::Volume, false, positions.clone());
        assert_eq!(leader_board.len(), 2);
        assert_eq!(leader_board.first().unwrap().volume, dec!(300));
        assert_eq!(leader_board.first().unwrap().rank, 1);
        assert_eq!(leader_board.first().unwrap().trader, trader_2);

        assert_eq!(leader_board.get(1).unwrap().volume, dec!(200));
        assert_eq!(leader_board.get(1).unwrap().rank, 2);
        assert_eq!(leader_board.get(1).unwrap().trader, trader_0);

        let leader_board = sort_leader_board(2, LeaderBoardCategory::Volume, true, positions);
        assert_eq!(leader_board.len(), 2);
        assert_eq!(leader_board.first().unwrap().volume, dec!(100));
        assert_eq!(leader_board.first().unwrap().rank, 1);
        assert_eq!(leader_board.first().unwrap().trader, trader_1);

        assert_eq!(leader_board.get(1).unwrap().volume, dec!(200));
        assert_eq!(leader_board.get(1).unwrap().rank, 2);
        assert_eq!(leader_board.get(1).unwrap().trader, trader_0);
    }

    fn create_dummy_position(trader: PublicKey, pnl: i64, quantity: f32) -> Position {
        Position {
            id: 0,
            contract_symbol: ContractSymbol::BtcUsd,
            trader_leverage: 0.0,
            quantity,
            trader_direction: Direction::Long,
            average_entry_price: 0.0,
            trader_liquidation_price: 0.0,
            coordinator_liquidation_price: 0.0,
            position_state: PositionState::Closed { pnl: 0 },
            coordinator_margin: 0,
            creation_timestamp: OffsetDateTime::now_utc(),
            expiry_timestamp: OffsetDateTime::now_utc(),
            update_timestamp: OffsetDateTime::now_utc(),
            trader,
            coordinator_leverage: 0.0,
            temporary_contract_id: None,
            closing_price: None,
            trader_margin: 0,
            stable: false,
            trader_realized_pnl_sat: Some(pnl),
            order_matching_fees: Amount::ZERO,
        }
    }

    fn leader_2() -> PublicKey {
        PublicKey::from_str("02d5aa8fce495f6301b466594af056a46104dcdc6d735ec4793aa43108854cbd4a")
            .unwrap()
    }

    fn leader_1() -> PublicKey {
        PublicKey::from_str("03b6fbc0de09815e2eb508feb8288ba6ac7f24aa27bd63435f6247d010334eaff2")
            .unwrap()
    }

    fn leader_0() -> PublicKey {
        PublicKey::from_str("0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166")
            .unwrap()
    }
}
