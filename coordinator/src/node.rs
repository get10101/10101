use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
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
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::task::JoinHandle;
use trade::cfd::calculate_margin;
use trade::Match;
use trade::MatchParams;
use trade::Trade;

pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node>,
    pub pending_trades: Arc<Mutex<HashSet<PublicKey>>>,
}

impl Node {
    /// Sets up the trade with the matched traders (maker, taker)
    pub async fn trade(&self, match_params: MatchParams) -> Result<()> {
        // todo: these proposals should be done in parallel with a timeout.
        // propose trade to the taker
        let taker_handle = self
            .propose_trade(&match_params.taker, &match_params.params)
            .await;

        // propose trade to the maker
        let maker_handle = self
            .propose_trade(&match_params.maker, &match_params.params)
            .await;

        // todo: add proper error handling. In case on of the proposals fails, but the other
        // succeeds we need to cancel the successful one.

        taker_handle
            .await
            .context("Failed to wait on future")?
            .context("Failed to propose trade to taker")?;

        maker_handle
            .await
            .context("Failed to wait on future")?
            .context("Failed to propose trade to maker")?;

        Ok(())
    }

    pub async fn propose_trade(
        &self,
        trade: &Trade,
        match_params: &Match,
    ) -> JoinHandle<Result<()>> {
        tokio::spawn({
            let match_params = match_params.clone();
            let trade = trade.clone();
            let node = self.inner.clone();
            async move {
                let maturity_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
                    + match_params.expiry.as_secs();

                let contract_symbol = match_params.contract_symbol.label();

                let margin_trader = calculate_margin(
                    match_params.execution_price,
                    match_params.quantity,
                    trade.leverage,
                );

                // The coordinator always trades at a leverage of 1
                let coordinator_leverage = 1.0;

                let margin_coordinator = calculate_margin(
                    match_params.execution_price,
                    match_params.quantity,
                    coordinator_leverage,
                );

                let total_collateral = margin_coordinator + margin_trader;

                // The contract input to be used for setting up the trade between the trader and the
                // coordinator
                let contract_input = ContractInput {
                    offer_collateral: margin_coordinator,
                    accept_collateral: margin_trader,
                    fee_rate: 2,
                    contract_infos: vec![ContractInputInfo {
                        contract_descriptor: dummy_contract_descriptor(total_collateral),
                        oracles: OracleInput {
                            public_keys: vec![node.oracle_pk()],
                            event_id: format!("{contract_symbol}{maturity_time}"),
                            threshold: 1,
                        },
                    }],
                };

                let channel_details = node.list_usable_channels();
                let channel_details = channel_details
                    .iter()
                    .find(|c| c.counterparty.node_id == trade.pub_key)
                    .context("Channel details not found")?;

                node.propose_dlc_channel(channel_details, &contract_input)
                    .await?;

                tracing::info!("Proposed dlc subchannel");
                Ok(())
            }
        })
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
