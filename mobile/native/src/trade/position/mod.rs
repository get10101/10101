use crate::trade::ContractSymbolTrade;
use crate::trade::DirectionTrade;
use bdk::bitcoin::secp256k1::PublicKey;
use bdk::bitcoin::XOnlyPublicKey;
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
use serde::Deserialize;
use serde::Serialize;

pub mod api;
pub mod handler;
pub mod subscriber;

#[derive(Debug, Clone)]
pub enum PositionStateTrade {
    /// The position is open
    ///
    /// Open in the sense, that there is an active position that is being rolled-over.
    /// Note that a "closed" position does not exist, but is just removed.
    /// During the process of getting closed (after creating the counter-order that will wipe out
    /// the position), the position is in state "Closing".
    ///
    /// Transitions:
    /// Open->Closing
    Open,
    /// The position is in the process of being closed
    ///
    /// The user has created an order that will wipe out the position.
    /// Once this order has been filled the "closed" the position is not shown in the user
    /// interface, so we don't have a "closed" state because no position data will be provided to
    /// the user interface.
    Closing,
}

#[derive(Debug, Clone)]
pub struct PositionTrade {
    pub leverage: f64,
    pub quantity: f64,
    pub contract_symbol: ContractSymbolTrade,
    pub direction: DirectionTrade,
    pub average_entry_price: f64,
    pub liquidation_price: f64,
    /// The unrealized PL can be positive or negative
    pub unrealized_pnl: i64,
    pub position_state: PositionStateTrade,
    pub collateral: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInput {
    pub maturity_time: u32,
    pub taker_margin: u64,
    pub maker_margin: u64,
    pub oracle_pk: XOnlyPublicKey,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeParams {
    pub taker_node_pubkey: PublicKey,
    pub contract_input: ContractInput,
}

impl From<ContractInput> for dlc_manager::contract::contract_input::ContractInput {
    fn from(contract_input: ContractInput) -> Self {
        // TODO: the contract symbol is currently hard coded, but should be fetched from the
        // contract input, once multiple contract symbols are supported.
        let contract_symbol = "btcusd";

        // TODO: calculate inverse payout curve.

        let total_collateral = contract_input.maker_margin + contract_input.taker_margin;
        dlc_manager::contract::contract_input::ContractInput {
            offer_collateral: contract_input.maker_margin,
            accept_collateral: contract_input.taker_margin,
            maturity_time: contract_input.maturity_time,
            fee_rate: 2,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor: ContractDescriptor::Numerical(NumericalDescriptor {
                    payout_function: PayoutFunction {
                        payout_function_pieces: vec![
                            PayoutFunctionPiece::PolynomialPayoutCurvePiece(
                                PolynomialPayoutCurvePiece::new(vec![
                                    PayoutPoint {
                                        event_outcome: 0,
                                        outcome_payout: 0,
                                        extra_precision: 0,
                                    },
                                    PayoutPoint {
                                        event_outcome: 50_000,
                                        outcome_payout: 0,
                                        extra_precision: 0,
                                    },
                                ])
                                .unwrap(),
                            ),
                            PayoutFunctionPiece::PolynomialPayoutCurvePiece(
                                PolynomialPayoutCurvePiece::new(vec![
                                    PayoutPoint {
                                        event_outcome: 50_000,
                                        outcome_payout: 0,
                                        extra_precision: 0,
                                    },
                                    PayoutPoint {
                                        event_outcome: 60_000,
                                        outcome_payout: total_collateral,
                                        extra_precision: 0,
                                    },
                                ])
                                .unwrap(),
                            ),
                            PayoutFunctionPiece::PolynomialPayoutCurvePiece(
                                PolynomialPayoutCurvePiece::new(vec![
                                    PayoutPoint {
                                        event_outcome: 60_000,
                                        outcome_payout: total_collateral,
                                        extra_precision: 0,
                                    },
                                    PayoutPoint {
                                        event_outcome: 1048575,
                                        outcome_payout: total_collateral,
                                        extra_precision: 0,
                                    },
                                ])
                                .unwrap(),
                            ),
                        ],
                    },
                    rounding_intervals: RoundingIntervals {
                        intervals: vec![RoundingInterval {
                            begin_interval: 0,
                            rounding_mod: 1,
                        }],
                    },
                    difference_params: None,
                    oracle_numeric_infos: dlc_trie::OracleNumericInfo {
                        base: 2,
                        nb_digits: vec![20],
                    },
                }),
                oracles: OracleInput {
                    public_keys: vec![contract_input.oracle_pk],
                    event_id: format!("{contract_symbol}{}", contract_input.maturity_time),
                    threshold: 1,
                },
            }],
        }
    }
}
