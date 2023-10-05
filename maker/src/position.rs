use anyhow::Result;
use async_trait::async_trait;
use hedging::derive_hedging_action;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::Add;
use std::time::Duration;
use uuid::Uuid;
use xtra::Mailbox;

mod bitmex;
mod hedging;
mod tentenone;

pub struct Manager {
    position: Position,
    bitmex_http_client: bitmex_client::client::Client,
}

#[async_trait]
impl xtra::Actor for Manager {
    type Stop = ();

    async fn started(&mut self, mailbox: &mut Mailbox<Self>) -> Result<(), Self::Stop> {
        tokio::spawn({
            let mailbox = mailbox.clone();
            async move {
                loop {
                    // We sleep first to allow the 10101 and BitMEX positions to be up-to-date
                    // before we start hedging.
                    tokio::time::sleep(Duration::from_secs(60)).await;

                    let _ = mailbox.address().send(Hedge).await;
                }
            }
        });

        Ok(())
    }

    async fn stopped(self) -> Self::Stop {}
}

impl Manager {
    pub fn new(bitmex_http_client: bitmex_client::client::Client) -> Self {
        Self {
            position: Position::new(),
            bitmex_http_client,
        }
    }

    /// Adjust hedging on _BitMEX_ based on the balance between the [`bitmex::Position`] and the
    /// [`tentenone::Position`].
    async fn hedge(&self, contract_symbol: &ContractSymbol) {
        let tentenone = self.position.get_tentenone(contract_symbol);

        // For the purposes of hedging we have to round to the number of 10101 contracts to the
        // nearest whole number.
        let tentenone = tentenone
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .to_i32()
            .expect("10101 position to fit in i32");

        let bitmex = self.position.get_bitmex(contract_symbol);

        let action = derive_hedging_action(tentenone, bitmex);

        if let Err(e) = self.create_bitmex_order(&action, contract_symbol).await {
            tracing::error!(
                ?action,
                "Failed to create order on BitMEX based on required hedging action: {e:#}"
            )
        }
    }

    async fn create_bitmex_order(
        &self,
        action: &hedging::Action,
        contract_symbol: &ContractSymbol,
    ) -> Result<()> {
        let (contracts, side) = match action.contracts() {
            0 => return Ok(()),
            n @ 1..=i32::MAX => (n.abs(), bitmex_client::models::Side::Buy),
            n @ i32::MIN..=-1 => (n.abs(), bitmex_client::models::Side::Sell),
        };

        tracing::info!(
            ?action,
            "Creating BitMEX order based on required hedging action"
        );

        let contract_symbol = match contract_symbol {
            ContractSymbol::BtcUsd => bitmex_client::models::ContractSymbol::XbtUsd,
        };

        self.bitmex_http_client
            .create_order(contract_symbol, contracts, side, None)
            .await?;

        Ok(())
    }
}

pub struct PositionUpdateTenTenOne(pub Vec<OrderTenTenOne>);

#[derive(Debug)]
pub struct OrderTenTenOne {
    order_id: Uuid,
    contract_symbol: ContractSymbol,
    /// The number of contracts corresponding to this position update.
    ///
    /// The sign determines the direction: positive is long; negative is short.
    contracts: Decimal,
}

impl OrderTenTenOne {
    pub fn new(order_id: Uuid, contract_symbol: ContractSymbol, contracts: Decimal) -> Self {
        Self {
            order_id,
            contract_symbol,
            contracts,
        }
    }
}

pub struct PositionUpdateBitmex {
    pub contract_symbol: ContractSymbol,
    pub contracts: i32,
}

pub struct GetPosition;

pub struct GetPositionResponse {
    pub tentenone: HashMap<ContractSymbol, Decimal>,
}

struct Hedge;

#[async_trait]
impl xtra::Handler<PositionUpdateTenTenOne> for Manager {
    type Return = ();

    async fn handle(
        &mut self,
        update: PositionUpdateTenTenOne,
        _: &mut xtra::Context<Self>,
    ) -> Self::Return {
        for OrderTenTenOne {
            order_id,
            contract_symbol,
            contracts,
        } in update.0
        {
            self.position
                .update_tentenone(contract_symbol, order_id, contracts);
        }
    }
}

#[async_trait]
impl xtra::Handler<PositionUpdateBitmex> for Manager {
    type Return = ();

    async fn handle(
        &mut self,
        update: PositionUpdateBitmex,
        _: &mut xtra::Context<Self>,
    ) -> Self::Return {
        self.position
            .update_bitmex(update.contract_symbol, update.contracts);
    }
}

#[async_trait]
impl xtra::Handler<GetPosition> for Manager {
    type Return = GetPositionResponse;

    async fn handle(&mut self, _: GetPosition, _: &mut xtra::Context<Self>) -> Self::Return {
        let tentenone =
            HashMap::from_iter(self.position.tentenone.clone().into_iter().map(|position| {
                (
                    position.contract_symbol(),
                    position.contracts().to_decimal(),
                )
            }));

        GetPositionResponse { tentenone }
    }
}

#[async_trait]
impl xtra::Handler<Hedge> for Manager {
    type Return = ();

    async fn handle(&mut self, _: Hedge, _: &mut xtra::Context<Self>) -> Self::Return {
        // TODO(lucas): Hedge for all `ContractSymbol` enum variants.
        self.hedge(&ContractSymbol::BtcUsd).await;
    }
}

/// The overall position of the maker.
#[derive(Debug)]
struct Position {
    tentenone: HashSet<tentenone::Position>,
    bitmex: HashSet<bitmex::Position>,
}

impl Position {
    pub fn new() -> Self {
        Self {
            tentenone: HashSet::from_iter([tentenone::Position::new(ContractSymbol::BtcUsd)]),
            bitmex: HashSet::from_iter([bitmex::Position::new(ContractSymbol::BtcUsd)]),
        }
    }

    fn update_tentenone(
        &mut self,
        contract_symbol: ContractSymbol,
        order_id: Uuid,
        contracts: Decimal,
    ) {
        let mut position = self
            .tentenone
            .get(&contract_symbol)
            .cloned()
            .unwrap_or(tentenone::Position::new(ContractSymbol::BtcUsd));

        position.update(order_id, contracts);

        self.tentenone.replace(position);
    }

    fn update_bitmex(&mut self, contract_symbol: ContractSymbol, contracts: i32) {
        let mut position = self
            .bitmex
            .get(&contract_symbol)
            .cloned()
            .unwrap_or(bitmex::Position::new(ContractSymbol::BtcUsd));

        position.update(contracts);

        self.bitmex.replace(position);
    }

    fn get_tentenone(&self, contract_symbol: &ContractSymbol) -> Decimal {
        match self.tentenone.get(contract_symbol) {
            Some(position) => match position.contracts() {
                Contracts::Balanced => Decimal::ZERO,
                Contracts::Long(contracts) => contracts,
                Contracts::Short(contracts) => -contracts,
            },
            None => Decimal::ZERO,
        }
    }

    fn get_bitmex(&self, contract_symbol: &ContractSymbol) -> i32 {
        match self.bitmex.get(contract_symbol) {
            Some(position) => position.contracts(),
            None => 0,
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Serialize)]
pub enum ContractSymbol {
    BtcUsd,
}

impl From<trade::ContractSymbol> for ContractSymbol {
    fn from(value: trade::ContractSymbol) -> Self {
        match value {
            trade::ContractSymbol::BtcUsd => Self::BtcUsd,
        }
    }
}

/// The number of contracts in the position, including their direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Contracts {
    /// Either zero contracts or the same number of long and short contracts.
    Balanced,
    /// More long contracts than short.
    Long(Decimal),
    /// More short contracts than long.
    Short(Decimal),
}

impl Contracts {
    fn new(contracts: Decimal) -> Self {
        if contracts.is_zero() {
            Self::Balanced
        } else if contracts.is_sign_positive() {
            Self::Long(contracts)
        } else {
            Self::Short(-contracts)
        }
    }

    fn to_decimal(self) -> Decimal {
        match self {
            Contracts::Balanced => Decimal::ZERO,
            Contracts::Long(contracts) => contracts,
            Contracts::Short(contracts) => -contracts,
        }
    }
}

impl Add for Contracts {
    type Output = Contracts;

    fn add(self, rhs: Self) -> Self::Output {
        let lhs = self.to_decimal();
        let rhs = rhs.to_decimal();

        Self::Output::new(lhs + rhs)
    }
}

impl Display for Contracts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Contracts::Balanced => "Balanced".to_string(),
            Contracts::Long(contracts) => format!("Long {contracts}"),
            Contracts::Short(contracts) => format!("Short {contracts}"),
        };

        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced() {
        let mut position = Position::new();
        let order_id = Uuid::new_v4();
        let contract_symbol = ContractSymbol::BtcUsd;
        let contracts = Decimal::ZERO;

        position.update_tentenone(contract_symbol, order_id, contracts);

        assert_eq!(position.get_tentenone(&contract_symbol), contracts);
    }

    #[test]
    fn long_100() {
        let mut position = Position::new();
        let order_id = Uuid::new_v4();
        let contract_symbol = ContractSymbol::BtcUsd;
        let contracts = Decimal::ONE_HUNDRED;

        position.update_tentenone(contract_symbol, order_id, contracts);

        assert_eq!(position.get_tentenone(&contract_symbol), contracts);
    }

    #[test]
    fn short_100() {
        let mut position = Position::new();
        let order_id = Uuid::new_v4();
        let contract_symbol = ContractSymbol::BtcUsd;
        let contracts = -Decimal::ONE_HUNDRED;

        position.update_tentenone(contract_symbol, order_id, contracts);

        assert_eq!(position.get_tentenone(&contract_symbol), contracts);
    }
}
