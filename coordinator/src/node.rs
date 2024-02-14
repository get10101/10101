use crate::db;
use crate::node::storage::NodeStorage;
use crate::orderbook::db::matches;
use crate::orderbook::db::orders;
use crate::position::models::Position;
use crate::position::models::PositionState;
use crate::storage::CoordinatorTenTenOneStorage;
use crate::trade::TradeExecutor;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use commons::MatchState;
use commons::OrderState;
use commons::TradeAndChannelParams;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::Connection;
use diesel::PgConnection;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::signed_channel::SignedChannelState;
use dlc_manager::channel::Channel;
use dlc_manager::DlcChannelId;
use dlc_messages::ChannelMessage;
use dlc_messages::Message;
use lightning::ln::ChannelId;
use lightning::util::config::UserConfig;
use ln_dlc_node::dlc_message::DlcMessage;
use ln_dlc_node::dlc_message::SerializedDlcMessage;
use ln_dlc_node::node;
use ln_dlc_node::node::dlc_message_name;
use ln_dlc_node::node::event::NodeEvent;
use ln_dlc_node::node::RunningNode;
use ln_dlc_node::WalletSettings;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;
use trade::cfd::calculate_pnl;
use trade::Direction;
use uuid::Uuid;

pub mod connection;
pub mod expired_positions;
pub mod rollover;
pub mod routing_fees;
pub mod storage;
pub mod unrealized_pnl;

#[derive(Debug, Clone)]
pub struct NodeSettings {
    // At times, we want to disallow opening new positions (e.g. before
    // scheduled upgrade)
    pub allow_opening_positions: bool,
    pub max_allowed_tx_fee_rate_when_opening_channel: Option<u32>,
    // Defines if we want to open jit channels
    pub jit_channels_enabled: bool,
    /// Defines the sats/vbyte to be used for all transactions within the sub-channel
    pub contract_tx_fee_rate: u64,
}

impl NodeSettings {
    fn to_wallet_settings(&self) -> WalletSettings {
        WalletSettings {
            max_allowed_tx_fee_rate_when_opening_channel: self
                .max_allowed_tx_fee_rate_when_opening_channel,
            jit_channels_enabled: self.jit_channels_enabled,
        }
    }
}

#[derive(Clone)]
pub struct Node {
    pub inner: Arc<node::Node<CoordinatorTenTenOneStorage, NodeStorage>>,
    _running: Arc<RunningNode>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    settings: Arc<RwLock<NodeSettings>>,
}

impl Node {
    pub fn new(
        inner: Arc<node::Node<CoordinatorTenTenOneStorage, NodeStorage>>,
        running: RunningNode,
        pool: Pool<ConnectionManager<PgConnection>>,
        settings: NodeSettings,
    ) -> Self {
        Self {
            inner,
            pool,
            settings: Arc::new(RwLock::new(settings)),
            _running: Arc::new(running),
        }
    }

    pub async fn update_settings(&self, settings: NodeSettings) {
        tracing::info!(?settings, "Updating node settings");
        *self.settings.write().await = settings.clone();

        // Forward relevant settings down to the wallet
        let wallet_settings = settings.to_wallet_settings();
        self.inner
            .ldk_wallet()
            .update_settings(wallet_settings)
            .await;
    }

    pub fn update_ldk_settings(&self, ldk_config: UserConfig) {
        self.inner.update_ldk_settings(ldk_config)
    }

    /// Returns true or false, whether we can find an usable channel with the provided trader.
    ///
    /// Note, we use the usable channel to implicitely check if the user is connected, as it
    /// wouldn't be usable otherwise.
    pub fn is_connected(&self, trader: &PublicKey) -> bool {
        let usable_channels = self.inner.channel_manager.list_usable_channels();
        let usable_channels = usable_channels
            .iter()
            .filter(|channel| channel.is_usable && channel.counterparty.node_id == *trader)
            .collect::<Vec<_>>();

        if usable_channels.len() > 1 {
            tracing::warn!(peer_id=%trader, "Found more than one usable channel with trader");
        }
        !usable_channels.is_empty()
    }

    pub async fn trade(&self, params: &TradeAndChannelParams) -> Result<()> {
        let mut connection = self.pool.get()?;

        let order_id = params.trade_params.filled_with.order_id;
        let trader_id = params.trade_params.pubkey;

        let trade_executor =
            TradeExecutor::new(self.inner.clone(), self.pool.clone(), self.settings.clone());
        match trade_executor.execute(params).await {
            Ok(()) => {
                tracing::info!(
                    %trader_id,
                    %order_id,
                    "Successfully processed match, setting match to Filled"
                );

                update_order_and_match(
                    &mut connection,
                    order_id,
                    MatchState::Filled,
                    OrderState::Taken,
                )?;
                Ok(())
            }
            Err(e) => {
                if let Err(e) = update_order_and_match(
                    &mut connection,
                    order_id,
                    MatchState::Failed,
                    OrderState::Failed,
                ) {
                    tracing::error!(%trader_id, %order_id, "Failed to update order and match: {e}");
                };

                Err(e).with_context(|| {
                    format!("Failed to trade with peer {trader_id} for order {order_id}")
                })
            }
        }
    }

    #[instrument(fields(position_id = position.id, trader_id = position.trader.to_string()),skip(self, conn, position))]
    pub fn finalize_closing_position(
        &self,
        conn: &mut PgConnection,
        position: Position,
    ) -> Result<()> {
        let trader_id = position.trader.to_string();
        tracing::debug!(?position, trader_id, "Finalize closing position",);

        let position_id = position.id;
        let temporary_contract_id = match position.temporary_contract_id {
            None => {
                tracing::error!("Position does not have temporary contract id");
                bail!("Position with id {position_id} with trader {trader_id} does not have temporary contract id");
            }
            Some(temporary_contract_id) => temporary_contract_id,
        };

        let contract = match self.inner.get_closed_contract(temporary_contract_id) {
            Ok(Some(closed_contract)) => closed_contract,
            Ok(None) => {
                tracing::error!("Subchannel not closed yet, skipping");
                bail!("Subchannel not closed for position {position_id} and trader {trader_id}");
            }
            Err(e) => {
                tracing::error!("Failed to get closed contract from DLC manager storage: {e:#}");
                bail!(e);
            }
        };

        let pnl = if let PositionState::Closing { closing_price } = position.position_state {
            let (initial_margin_long, initial_margin_short) = match position.trader_direction {
                Direction::Long => (position.trader_margin, position.coordinator_margin),
                Direction::Short => (position.coordinator_margin, position.trader_margin),
            };

            calculate_pnl(
                Decimal::from_f32(position.average_entry_price).expect("to fit into decimal"),
                Decimal::from_f32(closing_price).expect("to fit into decimal"),
                position.quantity,
                position.trader_direction,
                initial_margin_long as u64,
                initial_margin_short as u64,
            )?
        } else {
            -0
        };

        tracing::debug!(
            ?position,
            pnl = pnl,
            "Setting position to closed to match the contract state."
        );

        if let Err(e) =
            db::positions::Position::set_position_to_closed_with_pnl(conn, position.id, pnl)
        {
            tracing::error!(
                temporary_contract_id=%temporary_contract_id.to_hex(),
                pnl=contract.pnl,
                "Failed to set position to closed: {e:#}"
            )
        }
        Ok(())
    }

    pub fn process_incoming_dlc_messages(&self) {
        if !self
            .inner
            .dlc_message_handler
            .has_pending_messages_to_process()
        {
            return;
        }

        let messages = self
            .inner
            .dlc_message_handler
            .get_and_clear_received_messages();

        for (node_id, msg) in messages {
            let msg_name = dlc_message_name(&msg);
            if let Err(e) = self.process_dlc_message(node_id, msg) {
                tracing::error!(
                    from = %node_id,
                    kind = %msg_name,
                    "Failed to process DLC message: {e:#}"
                );
            }
        }
    }

    /// Process an incoming [`Message::Channel`] and update the 10101 position accordingly.
    ///
    /// - Any other kind of message will be ignored.
    /// - Any message that has already been processed will be skipped.
    ///
    /// Offers such as [`ChannelMessage::Offer`], [`ChannelMessage::SettleOffer`],
    /// [`ChannelMessage::CollaborativeCloseOffer`] and [`ChannelMessage::RenewOffer`] are
    /// automatically accepted. Unless the maturity date of the offer is already outdated.
    ///
    /// FIXME(holzeis): This function manipulates different data objects from different data sources
    /// and should use a transaction to make all changes atomic. Not doing so risks ending up in an
    /// inconsistent state. One way of fixing that could be to: (1) use a single data source for the
    /// 10101 data and the `rust-dlc` data; (2) wrap the function into a DB transaction which can be
    /// atomically rolled back on error or committed on success.
    fn process_dlc_message(&self, node_id: PublicKey, msg: Message) -> Result<()> {
        tracing::info!(
            from = %node_id,
            kind = %dlc_message_name(&msg),
            "Processing message"
        );

        let resp = match &msg {
            Message::OnChain(_) | Message::SubChannel(_) => {
                tracing::warn!(from = %node_id, kind = %dlc_message_name(&msg),"Ignoring unexpected dlc message.");
                None
            }
            Message::Channel(channel_msg) => {
                let inbound_msg = {
                    let mut conn = self.pool.get()?;
                    let serialized_inbound_message = SerializedDlcMessage::try_from(&msg)?;
                    let inbound_msg = DlcMessage::new(node_id, serialized_inbound_message, true)?;
                    match db::dlc_messages::get(&mut conn, &inbound_msg.message_hash)? {
                        Some(_) => {
                            tracing::debug!(%node_id, kind=%dlc_message_name(&msg), "Received message that has already been processed, skipping.");
                            return Ok(());
                        }
                        None => inbound_msg,
                    }
                };

                let resp = self
                    .inner
                    .dlc_manager
                    .on_dlc_message(&msg, node_id)
                    .with_context(|| {
                        format!(
                            "Failed to handle {} dlc message from {node_id}",
                            dlc_message_name(&msg)
                        )
                    })?;

                {
                    let mut conn = self.pool.get()?;
                    db::dlc_messages::insert(&mut conn, inbound_msg)?;
                }

                match channel_msg {
                    ChannelMessage::RenewFinalize(r) => {
                        // TODO: Receiving this message used to be specific to rolling over, but we
                        // now use the renew protocol for all (non-closing)
                        // trades beyond the first one.
                        // self.finalize_rollover(&r.channel_id)?;

                        let channel_id_hex_string = r.channel_id.to_hex();
                        tracing::info!(
                            channel_id = channel_id_hex_string,
                            node_id = node_id.to_string(),
                            "DLC channel renew protocol was finalized"
                        );

                        if self.is_in_rollover(node_id)? {
                            self.finalize_rollover(&r.channel_id)?;
                        } else {
                            let mut connection = self.pool.get()?;
                            db::positions::Position::update_proposed_position(
                                &mut connection,
                                node_id.to_string(),
                                PositionState::Open,
                            )?;
                        }
                    }
                    ChannelMessage::SettleFinalize(settle_finalize) => {
                        let channel_id_hex_string = settle_finalize.channel_id.to_hex();
                        tracing::info!(
                            channel_id = channel_id_hex_string,
                            node_id = node_id.to_string(),
                            "DLC channel settle protocol was finalized"
                        );
                        let mut connection = self.pool.get()?;

                        match db::positions::Position::get_position_by_trader(
                            &mut connection,
                            node_id,
                            vec![
                                // The price doesn't matter here.
                                PositionState::Closing { closing_price: 0.0 },
                            ],
                        )? {
                            None => {
                                tracing::error!(
                                    channel_id = channel_id_hex_string,
                                    "No position in Closing state found"
                                );
                            }
                            Some(position) => {
                                self.finalize_closing_position(&mut connection, position)?;
                            }
                        }
                    }
                    ChannelMessage::CollaborativeCloseOffer(close_offer) => {
                        let channel_id_hex_string = close_offer.channel_id.to_hex();
                        tracing::info!(
                            channel_id = channel_id_hex_string,
                            node_id = node_id.to_string(),
                            "Received an offer to collaboratively close a channel"
                        );

                        // TODO(bonomat): we should verify that the proposed amount is acceptable
                        self.inner
                            .accept_dlc_channel_collaborative_close(&close_offer.channel_id)?;
                    }
                    ChannelMessage::Accept(accept_channel) => {
                        let channel_id_hex_string = accept_channel.temporary_channel_id.to_hex();
                        tracing::info!(
                            channel_id = channel_id_hex_string,
                            node_id = node_id.to_string(),
                            "DLC channel open protocol was finalized"
                        );
                        let mut connection = self.pool.get()?;
                        db::positions::Position::update_proposed_position(
                            &mut connection,
                            node_id.to_string(),
                            PositionState::Open,
                        )?;
                    }
                    ChannelMessage::Reject(reject) => {
                        // TODO(holzeis): if an dlc channel gets rejected we have to deal with the
                        // counterparty as well.

                        let channel_id_hex_string = reject.channel_id.to_hex();

                        let channel = self.inner.get_dlc_channel_by_id(&reject.channel_id)?;
                        let mut connection = self.pool.get()?;

                        match channel {
                            Channel::Cancelled(_) => {
                                tracing::info!(
                                    channel_id = channel_id_hex_string,
                                    node_id = node_id.to_string(),
                                    "DLC Channel offer has been rejected. Setting position to failed."
                                );

                                db::positions::Position::update_proposed_position(
                                    &mut connection,
                                    node_id.to_string(),
                                    PositionState::Failed,
                                )?;
                            }
                            Channel::Signed(SignedChannel {
                                state: SignedChannelState::Established { .. },
                                ..
                            }) => {
                                // TODO(holzeis): Reverting the position state back from `Closing`
                                // to `Open` only works as long as we do not support resizing. This
                                // logic needs to be adapted when we implement resize.

                                tracing::info!(
                                    channel_id = channel_id_hex_string,
                                    node_id = node_id.to_string(),
                                    "DLC Channel settle offer has been rejected. Setting position to back to open."
                                );

                                db::positions::Position::update_closing_position(
                                    &mut connection,
                                    node_id.to_string(),
                                    PositionState::Open,
                                )?;
                            }
                            Channel::Signed(SignedChannel {
                                state: SignedChannelState::Settled { .. },
                                ..
                            }) => {
                                tracing::info!(
                                    channel_id = channel_id_hex_string,
                                    node_id = node_id.to_string(),
                                    "DLC Channel renew offer has been rejected. Setting position to failed."
                                );

                                db::positions::Position::update_proposed_position(
                                    &mut connection,
                                    node_id.to_string(),
                                    PositionState::Failed,
                                )?;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                };

                resp
            }
        };

        if let Some(msg) = resp {
            tracing::info!(
                to = %node_id,
                kind = %dlc_message_name(&msg),
                "Sending message"
            );

            self.inner
                .event_handler
                .publish(NodeEvent::SendDlcMessage { peer: node_id, msg })?;
        }

        Ok(())
    }
}

fn update_order_and_match(
    connection: &mut PgConnection,
    order_id: Uuid,
    match_state: MatchState,
    order_state: OrderState,
) -> Result<()> {
    connection
        .transaction(|connection| {
            matches::set_match_state(connection, order_id, match_state)?;

            orders::set_order_state(connection, order_id, order_state)?;

            diesel::result::QueryResult::Ok(())
        })
        .map_err(|e| anyhow!("Failed to update order and match. Error: {e:#}"))
}

pub enum TradeAction {
    OpenDlcChannel,
    OpenPosition(DlcChannelId),
    ClosePosition(DlcChannelId),
    ResizePosition(ChannelId),
}
