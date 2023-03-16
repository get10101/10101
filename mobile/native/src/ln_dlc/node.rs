use crate::api::Balances;
use crate::api::WalletInfo;
use crate::trade::order;
use crate::trade::position;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use dlc_messages::Message;
use dlc_messages::SubChannelMessage;
use ln_dlc_node::node::rust_dlc_manager::contract::Contract;
use ln_dlc_node::node::rust_dlc_manager::Storage;
use ln_dlc_node::node::sub_channel_message_as_str;
use ln_dlc_node::node::DlcManager;
use ln_dlc_node::Dlc;
use std::sync::Arc;

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<ln_dlc_node::node::Node>,
}

impl Node {
    pub fn get_wallet_info_from_node(&self) -> WalletInfo {
        WalletInfo {
            balances: Balances {
                lightning: self.inner.get_ldk_balance().available,
                on_chain: self
                    .inner
                    .get_on_chain_balance()
                    .expect("balance")
                    .confirmed,
            },
            history: vec![], // TODO: sync history
        }
    }

    pub fn process_incoming_messages(&self) -> Result<()> {
        let messages = self
            .inner
            .dlc_message_handler
            .get_and_clear_received_messages();

        for (node_id, msg) in messages {
            match msg {
                Message::OnChain(_) | Message::Channel(_) => {
                    tracing::debug!(from = %node_id, "Processing DLC-manager message");
                    let resp = self
                        .inner
                        .dlc_manager
                        .on_dlc_message(&msg, node_id)
                        .map_err(|e| anyhow!(e.to_string()))?;

                    if let Some(msg) = resp {
                        tracing::debug!(to = %node_id, "Sending DLC-manager message");
                        self.inner.dlc_message_handler.send_message(node_id, msg);
                    }
                }
                Message::SubChannel(incoming_msg) => {
                    tracing::debug!(
                        from = %node_id,
                        msg = %sub_channel_message_as_str(&incoming_msg),
                        "Processing DLC channel message"
                    );
                    let reply_msg = self
                        .inner
                        .sub_channel_manager
                        .on_sub_channel_message(&incoming_msg, &node_id)
                        .map_err(|e| anyhow!(e.to_string()))?;

                    if let SubChannelMessage::Offer(offer) = &incoming_msg {
                        let channel_id = offer.channel_id;

                        // TODO: We should probably verify that: (1) the counterparty is the
                        // coordinator and (2) the DLC channel offer is expected and correct.
                        if let Err(e) = self.inner.accept_dlc_channel_offer(&channel_id) {
                            tracing::error!(channel_id = %hex::encode(channel_id), "Failed to accept DLC channel offer: {e:#}");
                        }
                    }

                    if let SubChannelMessage::CloseOffer(offer) = &incoming_msg {
                        let channel_id = offer.channel_id;

                        // TODO: We should probably verify that: (1) the counterparty is the
                        // coordinator and (2) the DLC channel close offer is expected and correct.
                        if let Err(e) = self
                            .inner
                            .accept_dlc_channel_collaborative_settlement(&channel_id)
                        {
                            tracing::error!(channel_id = %hex::encode(channel_id), "Failed to accept DLC channel close offer: {e:#}");
                        }
                    }

                    if let Some(reply_msg) = reply_msg {
                        tracing::debug!(
                            to = %node_id,
                            msg = %sub_channel_message_as_str(&reply_msg),
                            "Sending DLC channel message"
                        );
                        self.inner
                            .dlc_message_handler
                            .send_message(node_id, Message::SubChannel(reply_msg.clone()));

                        if let SubChannelMessage::Finalize(_) = reply_msg {
                            let offer_collateral =
                                get_first_confirmed_dlc(&self.inner.dlc_manager)?.offer_collateral;

                            let filled_order = match order::handler::order_filled() {
                                Ok(filled_order) => filled_order,
                                Err(e) => {
                                    tracing::error!("Critical Error! We have a DLC but were unable to set the order to filled: {e:#}");
                                    continue;
                                }
                            };

                            if let Err(e) =
                                position::handler::position_update(filled_order, offer_collateral)
                            {
                                tracing::error!(
                                    "Failed to handle position after receiving DLC: {e:#}"
                                );
                                continue;
                            }
                        }
                    }
                }
            }
        }

        // NOTE: According to the docs of `process_events` we shouldn't have to call this since we
        // use `lightning-net-tokio`. But we copied this from `p2pderivatives/ldk-sample`
        if self.inner.dlc_message_handler.has_pending_messages() {
            self.inner.peer_manager.process_events();
        }

        Ok(())
    }
}

pub fn get_first_confirmed_dlc(dlc_manager: &DlcManager) -> Result<Dlc> {
    let confirmed_dlcs = dlc_manager
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
            accept_pk: signed.accepted_contract.offered_contract.counter_party,
        })
        .collect::<Vec<_>>();

    confirmed_dlcs
        .first()
        .context("No confirmed DLC found")
        .copied()
}
