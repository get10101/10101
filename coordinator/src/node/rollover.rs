use crate::check_version::check_version;
use crate::db;
use crate::db::positions;
use crate::dlc_protocol;
use crate::dlc_protocol::DlcProtocolType;
use crate::dlc_protocol::ProtocolId;
use crate::message::OrderbookMessage;
use crate::node::Node;
use crate::position::models::PositionState;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::XOnlyPublicKey;
use bitcoin::Network;
use commons::Message;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use dlc_manager::contract::contract_input::ContractInput;
use dlc_manager::contract::contract_input::ContractInputInfo;
use dlc_manager::contract::contract_input::OracleInput;
use dlc_manager::contract::Contract;
use dlc_manager::contract::ContractDescriptor;
use dlc_manager::DlcChannelId;
use futures::future::RemoteHandle;
use futures::FutureExt;
use ln_dlc_node::bitcoin_conversion::to_secp_pk_30;
use ln_dlc_node::bitcoin_conversion::to_xonly_pk_29;
use ln_dlc_node::bitcoin_conversion::to_xonly_pk_30;
use ln_dlc_node::node::event::NodeEvent;
use std::str::FromStr;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;
use trade::ContractSymbol;

#[derive(Debug, Clone)]
struct Rollover {
    counterparty_pubkey: PublicKey,
    contract_descriptor: ContractDescriptor,
    margin_coordinator: u64,
    margin_trader: u64,
    contract_symbol: ContractSymbol,
    oracle_pk: XOnlyPublicKey,
    contract_tx_fee_rate: u64,
    network: Network,
}

pub fn monitor(
    pool: Pool<ConnectionManager<PgConnection>>,
    mut receiver: broadcast::Receiver<NodeEvent>,
    notifier: mpsc::Sender<OrderbookMessage>,
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

impl Rollover {
    pub fn new(contract: Contract, network: Network) -> Result<Self> {
        let contract = match contract {
            Contract::Confirmed(contract) => contract,
            _ => bail!(
                "Cannot rollover a contract that is not confirmed. {:?}",
                contract
            ),
        };

        let offered_contract = contract.accepted_contract.offered_contract;
        let contract_info = offered_contract
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

        let margin_coordinator = offered_contract.offer_params.collateral;
        let margin_trader = offered_contract.total_collateral - margin_coordinator;

        let contract_tx_fee_rate = offered_contract.fee_rate_per_vb;
        Ok(Rollover {
            counterparty_pubkey: to_secp_pk_30(offered_contract.counter_party),
            contract_descriptor: contract_info.clone().contract_descriptor,
            margin_coordinator,
            margin_trader,
            oracle_pk: to_xonly_pk_30(oracle_announcement.oracle_public_key),
            contract_symbol: ContractSymbol::from_str(
                &oracle_announcement.oracle_event.event_id[..6],
            )?,
            contract_tx_fee_rate,
            network,
        })
    }

    pub fn event_id(&self) -> String {
        let maturity_time = self.maturity_time().unix_timestamp();
        format!("{}{maturity_time}", self.contract_symbol)
    }

    /// Calculates the maturity time based on the current expiry timestamp.
    pub fn maturity_time(&self) -> OffsetDateTime {
        commons::calculate_next_expiry(OffsetDateTime::now_utc(), self.network)
    }
}

impl Node {
    async fn check_if_eligible_for_rollover(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
        notifier: mpsc::Sender<OrderbookMessage>,
        trader_id: PublicKey,
        network: Network,
    ) -> Result<()> {
        let mut conn = spawn_blocking(move || pool.get())
            .await
            .expect("task to complete")?;

        tracing::debug!(%trader_id, "Checking if the users positions is eligible for rollover");

        if check_version(&mut conn, &trader_id).is_err() {
            tracing::info!(%trader_id, "User is not on the latest version. Skipping check if users position is eligible for rollover");
            return Ok(());
        }

        if let Some(position) = positions::Position::get_position_by_trader(
            &mut conn,
            trader_id,
            vec![PositionState::Open, PositionState::Rollover],
        )? {
            let signed_channel = self
                .inner
                .get_signed_channel_by_trader_id(position.trader)?;

            let contract_id = signed_channel.get_contract_id();

            if commons::is_eligible_for_rollover(OffsetDateTime::now_utc(), network)
                && !position.is_expired()
            {
                let next_expiry =
                    commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);
                if position.expiry_timestamp == next_expiry {
                    tracing::trace!(%trader_id, position_id=position.id, "Position has already been rolled over");
                    return Ok(());
                }

                tracing::debug!(%trader_id, position_id=position.id, "Proposing to rollover user's position");

                let message = OrderbookMessage::TraderMessage {
                    trader_id,
                    message: Message::Rollover(contract_id.map(hex::encode)),
                    // Ignore push notifying the user for that message as this is anyways only
                    // triggered when the user just connected to the websocket
                    // and we have a separate task that is push notifying the
                    // user if the rollover window is about to start.
                    notification: None,
                };
                if let Err(e) = notifier.send(message).await {
                    tracing::debug!("Failed to notify trader. Error: {e:#}");
                }
            }
        }

        Ok(())
    }

    /// Initiates the rollover protocol with the app.
    pub async fn propose_rollover(
        &self,
        dlc_channel_id: &DlcChannelId,
        network: Network,
    ) -> Result<()> {
        let contract = self.inner.get_contract_by_dlc_channel_id(dlc_channel_id)?;
        let rollover = Rollover::new(contract, network)?;
        let protocol_id = ProtocolId::new();

        tracing::debug!(node_id=%rollover.counterparty_pubkey, %protocol_id, "Rollover dlc channel");

        let contract_input: ContractInput = rollover.clone().into();

        let channel = self.inner.get_dlc_channel_by_id(dlc_channel_id)?;
        let previous_id = match channel.get_reference_id() {
            Some(reference_id) => Some(ProtocolId::try_from(reference_id)?),
            None => None,
        };

        let contract_id = self
            .inner
            .propose_dlc_channel_update(dlc_channel_id, contract_input, protocol_id.into())
            .await?;

        let protocol_executor = dlc_protocol::DlcProtocolExecutor::new(self.pool.clone());
        protocol_executor.start_dlc_protocol(
            protocol_id,
            previous_id,
            &contract_id,
            dlc_channel_id,
            DlcProtocolType::Rollover {
                trader: rollover.counterparty_pubkey,
            },
        )?;

        // Sets the position state to rollover indicating that a rollover is in progress.
        let mut connection = self.pool.get()?;
        db::positions::Position::rollover_position(
            &mut connection,
            rollover.counterparty_pubkey.to_string(),
            &rollover.maturity_time(),
        )
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

impl From<Rollover> for ContractInput {
    fn from(rollover: Rollover) -> Self {
        ContractInput {
            offer_collateral: rollover.margin_coordinator,
            accept_collateral: rollover.margin_trader,
            fee_rate: rollover.contract_tx_fee_rate,
            contract_infos: vec![ContractInputInfo {
                contract_descriptor: rollover.clone().contract_descriptor,
                oracles: OracleInput {
                    public_keys: vec![to_xonly_pk_29(rollover.oracle_pk)],
                    event_id: rollover.event_id(),
                    threshold: 1,
                },
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::absolute;
    use bitcoin::Transaction;
    use dlc::DlcTransactions;
    use dlc::PartyParams;
    use dlc_manager::contract::accepted_contract::AcceptedContract;
    use dlc_manager::contract::contract_info::ContractInfo;
    use dlc_manager::contract::enum_descriptor::EnumDescriptor;
    use dlc_manager::contract::offered_contract::OfferedContract;
    use dlc_manager::contract::signed_contract::SignedContract;
    use dlc_messages::oracle_msgs::EnumEventDescriptor;
    use dlc_messages::oracle_msgs::EventDescriptor;
    use dlc_messages::oracle_msgs::OracleAnnouncement;
    use dlc_messages::oracle_msgs::OracleEvent;
    use dlc_messages::FundingSignatures;
    use ln_dlc_node::bitcoin_conversion::to_secp_pk_29;
    use ln_dlc_node::bitcoin_conversion::to_tx_29;
    use ln_dlc_node::bitcoin_conversion::to_xonly_pk_29;
    use rand::Rng;

    #[test]
    fn test_new_rollover_from_signed_contract() {
        let expiry_timestamp = OffsetDateTime::now_utc().unix_timestamp() + 10_000;
        let contract = dummy_signed_contract(200, 100, expiry_timestamp as u32);
        let rollover = Rollover::new(Contract::Confirmed(contract), Network::Bitcoin).unwrap();
        assert_eq!(rollover.contract_symbol, ContractSymbol::BtcUsd);
        assert_eq!(rollover.margin_trader, 100);
        assert_eq!(rollover.margin_coordinator, 200);
    }

    #[test]
    fn test_new_rollover_from_other_contract() {
        let expiry_timestamp = OffsetDateTime::now_utc().unix_timestamp() + 10_000;
        assert!(Rollover::new(
            Contract::Offered(dummy_offered_contract(200, 100, expiry_timestamp as u32)),
            Network::Bitcoin
        )
        .is_err())
    }

    #[test]
    fn test_from_rollover_to_contract_input() {
        let margin_trader = 123;
        let margin_coordinator = 234;
        let rollover = Rollover {
            counterparty_pubkey: dummy_pubkey(),
            contract_descriptor: dummy_contract_descriptor(),
            margin_coordinator,
            margin_trader,
            contract_symbol: ContractSymbol::BtcUsd,
            oracle_pk: XOnlyPublicKey::from(dummy_pubkey()),
            contract_tx_fee_rate: 1,
            network: Network::Bitcoin,
        };

        let contract_input: ContractInput = rollover.into();
        assert_eq!(contract_input.accept_collateral, margin_trader);
        assert_eq!(contract_input.offer_collateral, margin_coordinator);
        assert_eq!(contract_input.contract_infos.len(), 1);
    }

    #[test]
    fn test_rollover_expired_position() {
        let expiry_timestamp = OffsetDateTime::now_utc().unix_timestamp() - 10_000;
        assert!(Rollover::new(
            Contract::Confirmed(dummy_signed_contract(200, 100, expiry_timestamp as u32)),
            Network::Bitcoin
        )
        .is_err())
    }

    fn dummy_signed_contract(
        margin_coordinator: u64,
        margin_trader: u64,
        expiry_timestamp: u32,
    ) -> SignedContract {
        SignedContract {
            accepted_contract: AcceptedContract {
                offered_contract: dummy_offered_contract(
                    margin_coordinator,
                    margin_trader,
                    expiry_timestamp,
                ),
                accept_params: dummy_params(margin_trader),
                funding_inputs: vec![],
                adaptor_infos: vec![],
                adaptor_signatures: None,
                dlc_transactions: DlcTransactions {
                    fund: to_tx_29(dummy_tx()),
                    cets: vec![],
                    refund: to_tx_29(dummy_tx()),
                    funding_script_pubkey: bitcoin_old::Script::new(),
                },
                accept_refund_signature: dummy_signature(),
            },
            adaptor_signatures: None,
            offer_refund_signature: dummy_signature(),
            funding_signatures: FundingSignatures {
                funding_signatures: vec![],
            },
            channel_id: None,
        }
    }

    fn dummy_offered_contract(
        margin_coordinator: u64,
        margin_trader: u64,
        expiry_timestamp: u32,
    ) -> OfferedContract {
        OfferedContract {
            id: dummy_id(),
            is_offer_party: false,
            contract_info: vec![ContractInfo {
                contract_descriptor: dummy_contract_descriptor(),
                oracle_announcements: vec![OracleAnnouncement {
                    announcement_signature: dummy_schnorr_signature(),
                    oracle_public_key: to_xonly_pk_29(XOnlyPublicKey::from(dummy_pubkey())),
                    oracle_event: OracleEvent {
                        oracle_nonces: vec![],
                        event_maturity_epoch: expiry_timestamp,
                        event_descriptor: EventDescriptor::EnumEvent(EnumEventDescriptor {
                            outcomes: vec![],
                        }),
                        event_id: format!("btcusd{expiry_timestamp}"),
                    },
                }],
                threshold: 0,
            }],
            counter_party: to_secp_pk_29(dummy_pubkey()),
            offer_params: dummy_params(margin_coordinator),
            total_collateral: margin_coordinator + margin_trader,
            funding_inputs_info: vec![],
            fund_output_serial_id: 0,
            fee_rate_per_vb: 0,
            cet_locktime: 0,
            refund_locktime: 0,
        }
    }

    fn dummy_pubkey() -> PublicKey {
        PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
            .expect("valid pubkey")
    }

    fn dummy_contract_descriptor() -> ContractDescriptor {
        ContractDescriptor::Enum(EnumDescriptor {
            outcome_payouts: vec![],
        })
    }

    fn dummy_id() -> [u8; 32] {
        let mut rng = rand::thread_rng();
        let dummy_id: [u8; 32] = rng.gen();
        dummy_id
    }

    fn dummy_schnorr_signature() -> bitcoin_old::secp256k1::schnorr::Signature {
        bitcoin_old::secp256k1::schnorr::Signature::from_str(
            "84526253c27c7aef56c7b71a5cd25bebb66dddda437826defc5b2568bde81f0784526253c27c7aef56c7b71a5cd25bebb66dddda437826defc5b2568bde81f07",
        ).unwrap()
    }

    fn dummy_params(collateral: u64) -> PartyParams {
        PartyParams {
            collateral,
            change_script_pubkey: bitcoin_old::Script::new(),
            change_serial_id: 0,
            fund_pubkey: to_secp_pk_29(dummy_pubkey()),
            input_amount: 0,
            inputs: vec![],
            payout_script_pubkey: bitcoin_old::Script::new(),
            payout_serial_id: 0,
        }
    }

    fn dummy_tx() -> Transaction {
        Transaction {
            version: 1,
            lock_time: absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        }
    }

    fn dummy_signature() -> bitcoin_old::secp256k1::ecdsa::Signature {
        bitcoin_old::secp256k1::ecdsa::Signature::from_str(
            "304402202f2545f818a5dac9311157d75065156b141e5a6437e817d1d75f9fab084e46940220757bb6f0916f83b2be28877a0d6b05c45463794e3c8c99f799b774443575910d",
        ).unwrap()
    }
}
