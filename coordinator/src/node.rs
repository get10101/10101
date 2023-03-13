use anyhow::Context;
use anyhow::Result;
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
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use trade::cfd::calculate_margin;
use trade::TradeParams;

pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node>,
}

impl Node {
    pub async fn trade(&self, trade_params: TradeParams) -> Result<()> {
        // The coordinator always trades at a leverage of 1
        let coordinator_leverage = 1.0;

        let margin_coordinator = calculate_margin(
            trade_params.execution_price,
            trade_params.quantity,
            coordinator_leverage,
        );
        let margin_trader = calculate_margin(
            trade_params.execution_price,
            trade_params.quantity,
            trade_params.leverage,
        );

        let total_collateral = margin_coordinator + margin_trader;

        let maturity_time =
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + trade_params.expiry.as_secs();

        let contract_symbol = trade_params.contract_symbol.label();

        // The contract input to be used for setting up the trade between the trader and the
        // coordinator
        let contract_input = ContractInput {
            offer_collateral: margin_trader,
            accept_collateral: margin_coordinator,
            fee_rate: 2,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor: dummy_contract_descriptor(total_collateral),
                oracles: OracleInput {
                    public_keys: vec![trade_params.oracle_pk],
                    event_id: format!("{contract_symbol}{maturity_time}"),
                    threshold: 1,
                },
            }],
        };

        let channel_details = self.inner.list_usable_channels();
        let channel_details = channel_details
            .iter()
            .find(|c| c.counterparty.node_id == trade_params.pubkey)
            .context("Channel details not found")?;

        self.inner
            .propose_dlc_channel(channel_details, &contract_input)
            .await?;

        Ok(())
    }
}

// TODO: To be deleted once we configure a proper payout curve
pub(crate) fn dummy_contract_descriptor(total_collateral: u64) -> ContractDescriptor {
    ContractDescriptor::Numerical(NumericalDescriptor {
        payout_function: PayoutFunction::new(vec![
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
        ])
        .unwrap(),
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
    })
}
