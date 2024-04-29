use crate::db::models::ContractSymbol;
use crate::db::models::Direction;
use crate::schema::funding_fee_events;
use bitcoin::SignedAmount;
use diesel::prelude::*;
use diesel::Queryable;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use xxi_node::commons;

#[derive(Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = funding_fee_events)]
pub(crate) struct UnpaidFundingFeeEvent {
    contract_symbol: ContractSymbol,
    contracts: f32,
    direction: Direction,
    price: f32,
    fee: i64,
    due_date: i64,
}

#[derive(Queryable, Debug, Clone, PartialEq)]
#[diesel(table_name = funding_fee_events)]
pub(crate) struct FundingFeeEvent {
    id: i32,
    contract_symbol: ContractSymbol,
    contracts: f32,
    direction: Direction,
    price: f32,
    fee: i64,
    due_date: i64,
    paid_date: Option<i64>,
}

impl UnpaidFundingFeeEvent {
    pub fn insert(
        conn: &mut SqliteConnection,
        funding_fee_event: crate::trade::FundingFeeEvent,
    ) -> QueryResult<Option<crate::trade::FundingFeeEvent>> {
        let affected_rows = diesel::insert_into(funding_fee_events::table)
            .values(UnpaidFundingFeeEvent::from(funding_fee_event))
            .on_conflict((
                funding_fee_events::contract_symbol,
                funding_fee_events::due_date,
            ))
            .do_nothing()
            .execute(conn)?;

        if affected_rows >= 1 {
            Ok(Some(funding_fee_event))
        } else {
            Ok(None)
        }
    }

    pub fn mark_as_paid(
        conn: &mut SqliteConnection,
        contract_symbol: commons::ContractSymbol,
        since: OffsetDateTime,
    ) -> QueryResult<()> {
        diesel::update(funding_fee_events::table)
            .filter(
                funding_fee_events::contract_symbol
                    .eq(ContractSymbol::from(contract_symbol))
                    .and(funding_fee_events::due_date.ge(since.unix_timestamp()))
                    .and(funding_fee_events::paid_date.is_null()),
            )
            .set(funding_fee_events::paid_date.eq(OffsetDateTime::now_utc().unix_timestamp()))
            .execute(conn)?;

        Ok(())
    }
}

impl FundingFeeEvent {
    pub fn get_all(conn: &mut SqliteConnection) -> QueryResult<Vec<crate::trade::FundingFeeEvent>> {
        let funding_fee_events: Vec<FundingFeeEvent> = funding_fee_events::table.load(conn)?;

        let funding_fee_events = funding_fee_events
            .into_iter()
            .map(crate::trade::FundingFeeEvent::from)
            .collect();

        Ok(funding_fee_events)
    }
}

impl From<crate::trade::FundingFeeEvent> for UnpaidFundingFeeEvent {
    fn from(
        crate::trade::FundingFeeEvent {
            contract_symbol,
            contracts,
            direction,
            price,
            fee,
            due_date,
            // An unpaid funding fee event should not have a `paid_date`.
            paid_date: _,
        }: crate::trade::FundingFeeEvent,
    ) -> Self {
        Self {
            contract_symbol: contract_symbol.into(),
            contracts: contracts.to_f32().expect("to fit"),
            direction: direction.into(),
            price: price.to_f32().expect("to fit"),
            fee: fee.to_sat(),
            due_date: due_date.unix_timestamp(),
        }
    }
}

impl From<FundingFeeEvent> for crate::trade::FundingFeeEvent {
    fn from(
        FundingFeeEvent {
            id: _,
            contract_symbol,
            contracts,
            direction,
            price,
            fee,
            due_date,
            paid_date,
        }: FundingFeeEvent,
    ) -> Self {
        Self {
            contract_symbol: contract_symbol.into(),
            contracts: Decimal::try_from(contracts).expect("to fit"),
            direction: direction.into(),
            price: Decimal::try_from(price).expect("to fit"),
            fee: SignedAmount::from_sat(fee),
            due_date: OffsetDateTime::from_unix_timestamp(due_date).expect("valid"),
            paid_date: paid_date
                .map(OffsetDateTime::from_unix_timestamp)
                .transpose()
                .expect("valid"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::MIGRATIONS;
    use diesel::Connection;
    use diesel::SqliteConnection;
    use diesel_migrations::MigrationHarness;
    use itertools::Itertools;
    use rust_decimal_macros::dec;
    use time::ext::NumericalDuration;
    use time::OffsetDateTime;

    #[test]
    fn test_funding_fee_event() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();

        let contract_symbol = xxi_node::commons::ContractSymbol::BtcUsd;
        let due_date = OffsetDateTime::from_unix_timestamp(1_546_300_800).unwrap();
        let funding_fee_event = crate::trade::FundingFeeEvent::unpaid(
            contract_symbol,
            Decimal::ONE_HUNDRED,
            xxi_node::commons::Direction::Long,
            dec!(70_000),
            SignedAmount::from_sat(100),
            due_date,
        );

        UnpaidFundingFeeEvent::insert(&mut conn, funding_fee_event).unwrap();

        // Does nothing, since `contract_symbol` and `due_date` are the same.
        UnpaidFundingFeeEvent::insert(
            &mut conn,
            crate::trade::FundingFeeEvent {
                contracts: Decimal::ONE_THOUSAND,
                direction: xxi_node::commons::Direction::Short,
                price: dec!(35_000),
                fee: SignedAmount::from_sat(-1_000),
                ..funding_fee_event
            },
        )
        .unwrap();

        let funding_fee_event_2 = crate::trade::FundingFeeEvent::unpaid(
            contract_symbol,
            Decimal::ONE_HUNDRED,
            xxi_node::commons::Direction::Long,
            dec!(70_000),
            SignedAmount::from_sat(100),
            due_date - 60.minutes(),
        );

        UnpaidFundingFeeEvent::insert(&mut conn, funding_fee_event_2).unwrap();

        let funding_fee_events = FundingFeeEvent::get_all(&mut conn).unwrap();

        assert_eq!(funding_fee_events.len(), 2);
        assert!(funding_fee_events.contains(&funding_fee_event));
        assert!(funding_fee_events.contains(&funding_fee_event_2));

        // We only mark as paid the funding fee event which has a due date after the third argument
        // to `mark_as_paid`.
        UnpaidFundingFeeEvent::mark_as_paid(&mut conn, contract_symbol, due_date - 30.minutes())
            .unwrap();

        let funding_fee_events = FundingFeeEvent::get_all(&mut conn).unwrap();

        assert!(funding_fee_events
            .iter()
            .filter(|event| event.paid_date.is_some())
            .exactly_one()
            .is_ok());

        assert!(funding_fee_events
            .iter()
            .filter(|event| event.paid_date.is_none())
            .exactly_one()
            .is_ok());
    }
}
