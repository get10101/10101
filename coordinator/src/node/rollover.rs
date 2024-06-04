use crate::check_version::check_version;
use crate::db;
use crate::db::positions;
use crate::decimal_from_f32;
use crate::dlc_protocol;
use crate::dlc_protocol::RolloverParams;
use crate::funding_fee::funding_fee_from_funding_fee_events;
use crate::node::Node;
use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use crate::payout_curve::build_contract_descriptor;
use crate::position::models::Position;
use crate::position::models::PositionState;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::PgConnection;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::contract::Contract;
use dlc_manager::DlcChannelId;
use futures::future::RemoteHandle;
use futures::FutureExt;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use xxi_node::commons;
use xxi_node::node::event::NodeEvent;
use xxi_node::node::ProtocolId;

pub fn monitor(
    pool: Pool<ConnectionManager<PgConnection>>,
    mut receiver: broadcast::Receiver<NodeEvent>,
    notifier: mpsc::Sender<Notification>,
    network: Network,
    node: Node,
) -> RemoteHandle<()> {
    let (fut, remote_handle) = async move {
        loop {
            match receiver.recv().await {
                Ok(NodeEvent::Connected { peer }) => {
                    tokio::spawn({
                        let notifier = notifier.clone();
                        let node = node.clone();
                        let pool = pool.clone();
                        async move {
                            if let Err(e) = node
                                .check_if_eligible_for_rollover(pool, notifier, peer, network)
                                .await
                            {
                                tracing::error!(
                                    "Failed to check if eligible for rollover. Error: {e:#}"
                                );
                            }
                        }
                    });
                }
                Ok(_) => {} // ignoring other node events
                Err(RecvError::Closed) => {
                    tracing::error!("Node event sender died! Channel closed.");
                    break;
                }
                Err(RecvError::Lagged(skip)) => tracing::warn!(%skip,
                    "Lagging behind on node events."
                ),
            }
        }
    }
    .remote_handle();

    tokio::spawn(fut);

    remote_handle
}

impl Node {
    async fn check_if_eligible_for_rollover(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
        notifier: mpsc::Sender<Notification>,
        trader_id: PublicKey,
        network: Network,
    ) -> Result<()> {
        let mut conn = spawn_blocking(move || pool.get())
            .await
            .expect("task to complete")?;

        tracing::debug!(%trader_id, "Checking if the user's position is eligible for rollover");

        if check_version(&mut conn, &trader_id).is_err() {
            tracing::info!(
                %trader_id,
                "User is not on the latest version. \
                 Will not check if their position is eligible for rollover"
            );
            return Ok(());
        }

        let position = match positions::Position::get_position_by_trader(
            &mut conn,
            trader_id,
            vec![PositionState::Open, PositionState::Rollover],
        )? {
            Some(position) => position,
            None => return Ok(()),
        };

        self.check_rollover(&mut conn, position, network, &notifier, None)
            .await
    }

    pub async fn check_rollover(
        &self,
        connection: &mut PooledConnection<ConnectionManager<PgConnection>>,
        position: Position,
        network: Network,
        notifier: &mpsc::Sender<Notification>,
        notification: Option<NotificationKind>,
    ) -> Result<()> {
        let trader_id = position.trader;
        let expiry_timestamp = position.expiry_timestamp;

        let signed_channel = self.inner.get_signed_channel_by_trader_id(trader_id)?;

        if commons::is_eligible_for_rollover(OffsetDateTime::now_utc(), network)
            // not expired
            && OffsetDateTime::now_utc() < expiry_timestamp
        {
            let next_expiry = commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);
            if expiry_timestamp >= next_expiry {
                tracing::trace!(%trader_id, "Position has already been rolled over");
                return Ok(());
            }

            tracing::debug!(%trader_id, "Push notifying user about rollover");

            if let Some(notification) = notification {
                if let Err(e) = notifier
                    .send(Notification::new(trader_id, notification))
                    .await
                {
                    tracing::warn!("Failed to push notify trader. Error: {e:#}");
                }
            }

            if self.is_connected(trader_id) {
                tracing::info!(%trader_id, "Proposing to rollover DLC channel");
                self.propose_rollover(
                    connection,
                    &signed_channel.channel_id,
                    position,
                    self.inner.network,
                )
                .await?;
            } else {
                tracing::warn!(%trader_id, "Skipping rollover, user is not connected.");
            }
        }

        Ok(())
    }

    /// Initiates the rollover protocol with the app.
    pub async fn propose_rollover(
        &self,
        conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
        dlc_channel_id: &DlcChannelId,
        position: Position,
        network: Network,
    ) -> Result<()> {
        let trader_pubkey = position.trader;

        let next_expiry = commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);

        let (oracle_pk, contract_tx_fee_rate) = {
            let old_contract = self.inner.get_contract_by_dlc_channel_id(dlc_channel_id)?;

            let old_offered_contract = match old_contract {
                Contract::Confirmed(contract) => contract.accepted_contract.offered_contract,
                _ => bail!("Cannot rollover a contract that is not confirmed"),
            };

            let contract_info = old_offered_contract
                .contract_info
                .first()
                .context("contract info to exist on a signed contract")?;
            let oracle_announcement = contract_info
                .oracle_announcements
                .first()
                .context("oracle announcement to exist on signed contract")?;

            let expiry_timestamp = OffsetDateTime::from_unix_timestamp(
                oracle_announcement.oracle_event.event_maturity_epoch as i64,
            )?;

            if expiry_timestamp < OffsetDateTime::now_utc() {
                bail!("Cannot rollover an expired position");
            }

            (
                oracle_announcement.oracle_public_key,
                old_offered_contract.fee_rate_per_vb,
            )
        };

        let maintenance_margin_rate = { self.settings.read().await.maintenance_margin_rate };
        let maintenance_margin_rate =
            Decimal::try_from(maintenance_margin_rate).expect("to fit into decimal");

        let funding_fee_events =
            db::funding_fee_events::get_outstanding_fees(conn, trader_pubkey, position.id)?;

        let funding_fee = funding_fee_from_funding_fee_events(&funding_fee_events);

        let position = position.apply_funding_fee(funding_fee, maintenance_margin_rate);
        let (collateral_reserve_coordinator, collateral_reserve_trader) =
            self.apply_funding_fee_to_channel(*dlc_channel_id, funding_fee)?;

        let Position {
            coordinator_margin: margin_coordinator,
            trader_margin: margin_trader,
            coordinator_leverage: leverage_coordinator,
            trader_leverage: leverage_trader,
            coordinator_liquidation_price: liquidation_price_coordinator,
            trader_liquidation_price: liquidation_price_trader,
            ..
        } = position;

        let contract_descriptor = build_contract_descriptor(
            Decimal::try_from(position.average_entry_price).expect("to fit"),
            margin_coordinator,
            margin_trader,
            leverage_coordinator,
            leverage_trader,
            position.trader_direction,
            collateral_reserve_coordinator,
            collateral_reserve_trader,
            position.quantity,
            position.contract_symbol,
        )
        .context("Could not build contract descriptor")?;

        let next_event_id = format!(
            "{}{}",
            position.contract_symbol,
            next_expiry.unix_timestamp()
        );

        let new_contract_input = ContractInput {
            offer_collateral: (margin_coordinator + collateral_reserve_coordinator).to_sat(),
            accept_collateral: (margin_trader + collateral_reserve_trader).to_sat(),
            fee_rate: contract_tx_fee_rate,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![oracle_pk],
                    event_id: next_event_id,
                    threshold: 1,
                },
            }],
        };

        let protocol_id = ProtocolId::new();

        tracing::debug!(
            %trader_pubkey,
            %protocol_id,
            ?funding_fee,
            "DLC channel rollover"
        );

        let channel = self.inner.get_dlc_channel_by_id(dlc_channel_id)?;
        let previous_id = match channel.get_reference_id() {
            Some(reference_id) => Some(ProtocolId::try_from(reference_id)?),
            None => None,
        };

        let funding_fee_event_ids = funding_fee_events
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>();

        let funding_fee_events = funding_fee_events
            .into_iter()
            .map(xxi_node::message_handler::FundingFeeEvent::from)
            .collect();

        let temporary_contract_id = self
            .inner
            .propose_rollover(
                dlc_channel_id,
                new_contract_input,
                protocol_id.into(),
                funding_fee_events,
            )
            .await?;

        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
        protocol_executor
            .start_rollover(
                protocol_id,
                previous_id,
                &temporary_contract_id,
                dlc_channel_id,
                RolloverParams {
                    protocol_id,
                    trader_pubkey,
                    margin_coordinator,
                    margin_trader,
                    leverage_coordinator: decimal_from_f32(leverage_coordinator),
                    leverage_trader: decimal_from_f32(leverage_trader),
                    liquidation_price_coordinator: decimal_from_f32(liquidation_price_coordinator),
                    liquidation_price_trader: decimal_from_f32(liquidation_price_trader),
                    expiry_timestamp: next_expiry,
                },
                funding_fee_event_ids,
            )
            .context("Failed to insert start of rollover protocol in dlc_protocols table")?;

        db::positions::Position::rollover_position(conn, trader_pubkey, &next_expiry)
            .context("Failed to set position state to rollover")?;

        self.inner
            .event_handler
            .publish(NodeEvent::SendLastDlcMessage {
                peer: trader_pubkey,
            });

        Ok(())
    }

    pub fn is_in_rollover(&self, trader_id: PublicKey) -> Result<bool> {
        let mut conn = self.pool.get()?;
        let position = db::positions::Position::get_position_by_trader(
            &mut conn,
            trader_id,
            vec![PositionState::Rollover],
        )?;

        Ok(position.is_some())
    }
}
