use crate::node::DlcManager;
use crate::node::Node;
use crate::node::SubChannelManager;
use crate::DlcMessageHandler;
use crate::PeerManager;
use anyhow::anyhow;
use anyhow::Result;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::contract::numerical_descriptor::NumericalDescriptor;
use dlc_manager::contract::Contract;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::payout_curve::PayoutFunction;
use dlc_manager::payout_curve::PayoutFunctionPiece;
use dlc_manager::payout_curve::PayoutPoint;
use dlc_manager::payout_curve::PolynomialPayoutCurvePiece;
use dlc_manager::payout_curve::RoundingInterval;
use dlc_manager::payout_curve::RoundingIntervals;
use dlc_manager::Oracle;
use dlc_manager::Storage;
use dlc_messages::Message;
use dlc_messages::SubChannelMessage;
use lightning::ln::channelmanager::ChannelDetails;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use trade::cfd::calculate_margin;
use trade::TradeParams;

pub struct Dlc {
    pub id: [u8; 32],
    pub offer_collateral: u64,
    pub accept_collateral: u64,
}

impl Node {
    // TODO: This API doesn't belong in this crate!
    pub fn trade(&self, trade_params: TradeParams) -> Result<ContractInput> {
        let mut pending_trades = self
            .pending_trades
            .lock()
            .map_err(|e| anyhow!("Failed to access pending trades: {e:#}"))?;

        // TODO: We need to keep around more information than just the pubkey and have to introduce
        // validation steps once we add the maker
        pending_trades.insert(trade_params.pubkey);

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

        Ok(contract_input)
    }

    pub async fn propose_dlc_channel(
        &self,
        channel_details: &ChannelDetails,
        contract_input: &ContractInput,
    ) -> Result<()> {
        let announcement = tokio::task::spawn_blocking({
            let oracle = self.oracle.clone();
            let event_id = contract_input.contract_infos[0].oracles.event_id.clone();
            move || {
                oracle
                    .get_announcement(&event_id)
                    .map_err(|e| anyhow!(e.to_string()))
            }
        })
        .await??;

        let sub_channel_offer = self
            .sub_channel_manager
            .offer_sub_channel(
                &channel_details.channel_id,
                contract_input,
                &[vec![announcement]],
            )
            .unwrap();

        self.dlc_message_handler.send_message(
            channel_details.counterparty.node_id,
            Message::SubChannel(SubChannelMessage::Offer(sub_channel_offer)),
        );

        Ok(())
    }

    pub fn accept_dlc_channel_offer(&self, channel_id: &[u8; 32]) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel offer");

        let (node_id, accept_sub_channel) = self
            .sub_channel_manager
            .accept_sub_channel(channel_id)
            .map_err(|e| anyhow!(e.to_string()))?;

        self.dlc_message_handler.send_message(
            node_id,
            Message::SubChannel(SubChannelMessage::Accept(accept_sub_channel)),
        );

        Ok(())
    }

    pub fn propose_dlc_channel_collaborative_settlement(
        &self,
        channel_id: &[u8; 32],
        accept_settlement_amount: u64,
    ) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(
            channel_id = %channel_id_hex,
            %accept_settlement_amount,
            "Settling DLC channel collaboratively"
        );

        let (sub_channel_close_offer, counterparty_pk) = self
            .sub_channel_manager
            .offer_subchannel_close(channel_id, accept_settlement_amount)
            .map_err(|e| anyhow!("{e}"))?;

        self.dlc_message_handler.send_message(
            counterparty_pk,
            Message::SubChannel(SubChannelMessage::CloseOffer(sub_channel_close_offer)),
        );

        Ok(())
    }

    pub fn accept_dlc_channel_collaborative_settlement(&self, channel_id: &[u8; 32]) -> Result<()> {
        let channel_id_hex = hex::encode(channel_id);

        tracing::info!(channel_id = %channel_id_hex, "Accepting DLC channel collaborative settlement");

        let (sub_channel_close_accept, counterparty_pk) = self
            .sub_channel_manager
            .accept_subchannel_close_offer(channel_id)
            .map_err(|e| anyhow!(e.to_string()))?;

        self.dlc_message_handler.send_message(
            counterparty_pk,
            Message::SubChannel(SubChannelMessage::CloseAccept(sub_channel_close_accept)),
        );

        Ok(())
    }

    pub fn get_confirmed_dlcs(&self) -> Result<Vec<Dlc>> {
        let confimed_dlcs = self
            .dlc_manager
            .get_store()
            .get_contracts()
            .map_err(|e| anyhow!("Unable to get contracts from manager: {e:#}"))?
            .iter()
            .filter_map(|contract| match contract {
                Contract::Confirmed(signed) => Some((contract.get_id(), signed)),
                _ => None,
            })
            .map(|(id, signed)| Dlc {
                id,
                offer_collateral: signed
                    .accepted_contract
                    .offered_contract
                    .offer_params
                    .collateral,
                accept_collateral: signed.accepted_contract.accept_params.collateral,
            })
            .collect();

        Ok(confimed_dlcs)
    }

    pub fn process_incoming_messages(&self) -> Result<()> {
        Node::process_incoming_messages_internal(
            &self.dlc_message_handler,
            &self.dlc_manager,
            &self.sub_channel_manager,
            &self.peer_manager,
        )
    }

    pub(crate) fn process_incoming_messages_internal(
        dlc_message_handler: &DlcMessageHandler,
        dlc_manager: &DlcManager,
        sub_channel_manager: &SubChannelManager,
        peer_manager: &PeerManager,
    ) -> Result<()> {
        let messages = dlc_message_handler.get_and_clear_received_messages();

        for (node_id, msg) in messages {
            match msg {
                Message::OnChain(_) | Message::Channel(_) => {
                    tracing::debug!(from = %node_id, "Processing DLC-manager message");
                    let resp = dlc_manager
                        .on_dlc_message(&msg, node_id)
                        .map_err(|e| anyhow!(e.to_string()))?;

                    if let Some(msg) = resp {
                        tracing::debug!(to = %node_id, "Sending DLC-manager message");
                        dlc_message_handler.send_message(node_id, msg);
                    }
                }
                Message::SubChannel(msg) => {
                    tracing::debug!(
                        from = %node_id,
                        msg = %sub_channel_message_as_str(&msg),
                        "Processing sub-channel message"
                    );
                    let resp = sub_channel_manager
                        .on_sub_channel_message(&msg, &node_id)
                        .map_err(|e| anyhow!(e.to_string()))?;

                    if let Some(msg) = resp {
                        tracing::debug!(
                            to = %node_id,
                            msg = %sub_channel_message_as_str(&msg),
                            "Sending sub-channel message"
                        );
                        dlc_message_handler.send_message(node_id, Message::SubChannel(msg));
                    }
                }
            }
        }

        // NOTE: According to the docs of `process_events` we shouldn't have to call this since we
        // use `lightning-net-tokio`. But we copied this from `p2pderivatives/ldk-sample`
        if dlc_message_handler.has_pending_messages() {
            peer_manager.process_events();
        }

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

fn sub_channel_message_as_str(msg: &SubChannelMessage) -> &str {
    use SubChannelMessage::*;

    match msg {
        Offer(_) => "Offer",
        Accept(_) => "Accept",
        Confirm(_) => "Confirm",
        Finalize(_) => "Finalize",
        CloseOffer(_) => "CloseOffer",
        CloseAccept(_) => "CloseAccept",
        CloseConfirm(_) => "CloseConfirm",
        CloseFinalize(_) => "CloseFinalize",
        CloseReject(_) => "CloseReject",
    }
}
