use crate::db;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::ln_dlc::node::Node;
use crate::trade::order;
use crate::trade::position;
use crate::trade::position::Position;
use anyhow::Result;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannel;
use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannelState;
use ln_dlc_node::node::rust_dlc_manager::ChannelId;
use ln_dlc_node::node::rust_dlc_manager::Storage;
use std::time::Duration;

#[derive(PartialEq, Clone, Debug)]
enum SyncPositionToDlcAction {
    ContinueSubchannelProtocol,
    CreatePosition(ChannelId),
    RemovePosition,
}

impl Node {
    /// Syncs the position with the dlc channel state.
    ///
    /// TODO(holzeis): With https://github.com/get10101/10101/issues/530 we should not require this logic anymore.
    ///
    /// - DLC Channel in state `Signed` but no position: Create position from `filling` order.
    /// - DLC Channel in state `OffChainClosed` and a position exists. Delete the position.
    /// - DLC Channel in state `CloseOffered` or `CloseAccepted`: Inform the UI that the dlc channel
    ///   is recovering.
    /// - DLC Channel in state `Offered`, `Accepted` or `Finalized`: Inform the UI that the dlc
    ///   channel is recovering.
    /// - DLC Channel in any other state but with position: Delete position the channel might have
    ///   been force closed.
    pub async fn sync_position_with_dlc_channel_state(&self) -> Result<()> {
        let channels = self.inner.channel_manager.list_channels();
        let channel_details = match channels.first() {
            Some(channel_details) => channel_details,
            None => return Ok(()),
        };
        let dlc_channels = self.inner.dlc_manager.get_store().get_sub_channels()?;
        let dlc_channel = dlc_channels
            .iter()
            .find(|dlc_channel| dlc_channel.channel_id == channel_details.channel_id);

        let positions = db::get_positions()?;

        match determine_sync_position_to_dlc_action(&positions.first(), &dlc_channel) {
            Some(SyncPositionToDlcAction::ContinueSubchannelProtocol) => self.recover_dlc().await?,
            Some(SyncPositionToDlcAction::CreatePosition(channel_id)) => {
                match order::handler::order_filled() {
                    Ok(order) => {
                        let (accept_collateral, expiry_timestamp) = self
                            .inner
                            .get_collateral_and_expiry_for_confirmed_contract(channel_id)?;

                        position::handler::update_position_after_dlc_creation(
                            order,
                            accept_collateral,
                            expiry_timestamp,
                        )?;

                        tracing::info!("Successfully recovered position from order.");
                    }
                    Err(e) => {
                        tracing::error!("Could not recover position from order as no filling order was found! Error: {e:#}");
                    }
                }
            }
            Some(SyncPositionToDlcAction::RemovePosition) => {
                let filled_order = match order::handler::order_filled() {
                    Ok(filled_order) => Some(filled_order),
                    Err(_) => None,
                };

                position::handler::update_position_after_dlc_closure(filled_order)?;
            }
            None => (),
        }

        Ok(())
    }

    /// Sends a pending RecoverDlc background task notification to the UI, allowing the UI to show a
    /// dialog with a spinner that the DLC protocol is still in progress.
    /// Also triggers the `periodic_check` to process any actions that might have been created after
    /// the channel reestablishment.
    ///
    /// fixme(holzeis): We currently use different events for show the recovery of a dlc and the
    /// waiting for an order execution in the happy case (without an restart in between). Those
    /// events and dialogs should be aligned.
    async fn recover_dlc(&self) -> Result<()> {
        tracing::warn!("It looks like the app was closed while the protocol was still executing.");
        event::publish(&EventInternal::BackgroundNotification(
            BackgroundTask::RecoverDlc(TaskStatus::Pending),
        ));

        // fixme(holzeis): We are manually calling the periodic check here to speed up the
        // processing of pending actions.
        // Note, this might not speed up the process, as the coordinator might have to resend a
        // message to continue the protocol. This should be fixed in `rust-dlc` and any
        // pending actions should be processed immediately once the channel is ready instead
        // of periodically checking if a pending action needs to be sent.
        // Note, pending actions can only get created on channel reestablishment, hence we are
        // waiting for arbitrary 5 seconds here to ensure that the channel is reestablished.
        tokio::time::sleep(Duration::from_secs(5)).await;
        if let Err(e) = self.inner.sub_channel_manager_periodic_check().await {
            tracing::error!("Failed to process periodic check! Error: {e:#}");
        }

        Ok(())
    }
}

/// Determines the action required in case the position and the dlc state get out of sync.
///
/// Returns
/// - `Recover` if dlc is in an intermediate state.
/// - `CreatePosition` if the dlc is `Signed`, but no position exists.
/// - `RemovePosition` if the dlc is in any other state than `Signed` or an intermediate state and a
///   position exists.
/// - `None` otherwise.
fn determine_sync_position_to_dlc_action(
    position: &Option<&Position>,
    dlc_channel: &Option<&SubChannel>,
) -> Option<SyncPositionToDlcAction> {
    match (position, dlc_channel) {
        (Some(_), Some(dlc_channel)) => {
            if matches!(dlc_channel.state, SubChannelState::Signed(_)) {
                tracing::debug!("DLC channel and position are in sync");
                None
            } else {
                tracing::warn!(dlc_channel_state=?dlc_channel.state, "Found unexpected sub channel state");
                if matches!(dlc_channel.state, SubChannelState::OffChainClosed) {
                    tracing::warn!("Deleting position as dlc channel is already closed!");
                    Some(SyncPositionToDlcAction::RemovePosition)
                } else if matches!(
                    dlc_channel.state,
                    SubChannelState::CloseOffered(_) | SubChannelState::CloseAccepted(_)
                ) {
                    Some(SyncPositionToDlcAction::ContinueSubchannelProtocol)
                } else {
                    tracing::warn!(
                        "The DLC is in a state that can not be recovered. Removing position."
                    );
                    // maybe a left over after a force-closure
                    Some(SyncPositionToDlcAction::RemovePosition)
                }
            }
        }
        (None, Some(dlc_channel)) => {
            if matches!(dlc_channel.state, SubChannelState::OffChainClosed) {
                tracing::debug!("DLC channel and position are in sync");
                None
            } else {
                tracing::warn!(dlc_channel_state=?dlc_channel.state, "Found unexpected sub channel state");
                if matches!(dlc_channel.state, SubChannelState::Signed(_)) {
                    tracing::warn!("Trying to recover position from order");
                    Some(SyncPositionToDlcAction::CreatePosition(
                        dlc_channel.channel_id,
                    ))
                } else if matches!(
                    dlc_channel.state,
                    SubChannelState::Offered(_)
                        | SubChannelState::Accepted(_)
                        | SubChannelState::Finalized(_)
                ) {
                    Some(SyncPositionToDlcAction::ContinueSubchannelProtocol)
                } else {
                    tracing::warn!("The DLC is in a state that can not be recovered.");
                    None
                }
            }
        }
        (Some(_), None) => {
            tracing::warn!("Found position but without dlc channel. Removing position");
            Some(SyncPositionToDlcAction::RemovePosition)
        }
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use crate::ln_dlc::sync_position_to_dlc::determine_sync_position_to_dlc_action;
    use crate::ln_dlc::sync_position_to_dlc::SyncPositionToDlcAction;
    use crate::trade::position::Position;
    use crate::trade::position::PositionState;
    use bitcoin::secp256k1::ecdsa::Signature;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::PackedLockTime;
    use bitcoin::Transaction;
    use dlc::channel::sub_channel::SplitTx;
    use lightning::chain::transaction::OutPoint;
    use lightning::ln::chan_utils::CounterpartyCommitmentSecrets;
    use ln_dlc_node::node::rust_dlc_manager::channel::party_points::PartyBasePoints;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::AcceptedSubChannel;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::CloseAcceptedSubChannel;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::CloseOfferedSubChannel;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::LnRollBackInfo;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::OfferedSubChannel;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::SignedSubChannel;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannel;
    use ln_dlc_node::node::rust_dlc_manager::subchannel::SubChannelState;
    use ln_dlc_node::node::rust_dlc_manager::ChannelId;
    use secp256k1_zkp::EcdsaAdaptorSignature;
    use std::str::FromStr;
    use time::OffsetDateTime;
    use trade::ContractSymbol;
    use trade::Direction;

    #[test]
    fn test_none_position_and_none_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(&None, &None);
        assert_eq!(None, action);
    }

    #[test]
    fn test_some_position_and_none_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(&Some(&get_dummy_position()), &None);
        assert_eq!(Some(SyncPositionToDlcAction::RemovePosition), action);
    }

    #[test]
    fn test_some_position_and_offchainclosed_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(&Some(&get_dummy_position()), &None);
        assert_eq!(Some(SyncPositionToDlcAction::RemovePosition), action);
    }

    #[test]
    fn test_some_position_and_signed_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &Some(&get_dummy_position()),
            &Some(&get_dummy_dlc_channel(SubChannelState::Signed(
                get_dummy_signed_sub_channel(),
            ))),
        );
        assert_eq!(None, action);
    }

    #[test]
    fn test_some_position_and_closeoffered_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &Some(&get_dummy_position()),
            &Some(&get_dummy_dlc_channel(SubChannelState::CloseOffered(
                get_dummy_close_offered_sub_channel(),
            ))),
        );
        assert_eq!(
            Some(SyncPositionToDlcAction::ContinueSubchannelProtocol),
            action
        );
    }

    #[test]
    fn test_some_position_and_closeaccepted_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &Some(&get_dummy_position()),
            &Some(&get_dummy_dlc_channel(SubChannelState::CloseAccepted(
                get_dummy_close_accepted_sub_channel(),
            ))),
        );
        assert_eq!(
            Some(SyncPositionToDlcAction::ContinueSubchannelProtocol),
            action
        );
    }

    #[test]
    fn test_none_position_and_offchainclosed_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &None,
            &Some(&get_dummy_dlc_channel(SubChannelState::OffChainClosed)),
        );
        assert_eq!(None, action);
    }

    #[test]
    fn test_none_position_and_signed_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &None,
            &Some(&get_dummy_dlc_channel(SubChannelState::Signed(
                get_dummy_signed_sub_channel(),
            ))),
        );
        assert!(matches!(
            action,
            Some(SyncPositionToDlcAction::CreatePosition(_))
        ));
    }

    #[test]
    fn test_none_position_and_offered_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &None,
            &Some(&get_dummy_dlc_channel(SubChannelState::Offered(
                get_dummy_offered_sub_channel(),
            ))),
        );
        assert_eq!(
            Some(SyncPositionToDlcAction::ContinueSubchannelProtocol),
            action
        );
    }

    #[test]
    fn test_none_position_and_accepted_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &None,
            &Some(&get_dummy_dlc_channel(SubChannelState::Accepted(
                get_dummy_accepted_sub_channel(),
            ))),
        );
        assert_eq!(
            Some(SyncPositionToDlcAction::ContinueSubchannelProtocol),
            action
        );
    }

    #[test]
    fn test_none_position_and_finalized_dlc_channel() {
        let action = determine_sync_position_to_dlc_action(
            &None,
            &Some(&get_dummy_dlc_channel(SubChannelState::Finalized(
                get_dummy_signed_sub_channel(),
            ))),
        );
        assert_eq!(
            Some(SyncPositionToDlcAction::ContinueSubchannelProtocol),
            action
        );
    }

    #[test]
    fn test_none_position_and_other_dlc_channel_state() {
        let action = determine_sync_position_to_dlc_action(
            &None,
            &Some(&get_dummy_dlc_channel(SubChannelState::OnChainClosed)),
        );
        assert_eq!(None, action);
    }

    #[test]
    fn test_some_position_and_other_dlc_channel_state() {
        let action = determine_sync_position_to_dlc_action(
            &Some(&get_dummy_position()),
            &Some(&get_dummy_dlc_channel(SubChannelState::OnChainClosed)),
        );
        assert_eq!(Some(SyncPositionToDlcAction::RemovePosition), action);
    }

    fn get_dummy_position() -> Position {
        Position {
            leverage: 0.0,
            quantity: 0.0,
            contract_symbol: ContractSymbol::BtcUsd,
            direction: Direction::Long,
            average_entry_price: 0.0,
            liquidation_price: 0.0,
            position_state: PositionState::Open,
            collateral: 0,
            expiry: OffsetDateTime::now_utc(),
            updated: OffsetDateTime::now_utc(),
            created: OffsetDateTime::now_utc(),
        }
    }

    fn get_dummy_dlc_channel(state: SubChannelState) -> SubChannel {
        SubChannel {
            channel_id: ChannelId::default(),
            counter_party: get_dummy_pubkey(),
            update_idx: 0,
            state,
            per_split_seed: None,
            fee_rate_per_vb: 0,
            own_base_points: PartyBasePoints {
                own_basepoint: get_dummy_pubkey(),
                revocation_basepoint: get_dummy_pubkey(),
                publish_basepoint: get_dummy_pubkey(),
            },
            counter_base_points: None,
            fund_value_satoshis: 0,
            original_funding_redeemscript: Default::default(),
            is_offer: false,
            own_fund_pk: get_dummy_pubkey(),
            counter_fund_pk: get_dummy_pubkey(),
            counter_party_secrets: CounterpartyCommitmentSecrets::new(),
        }
    }

    fn get_dummy_signed_sub_channel() -> SignedSubChannel {
        SignedSubChannel {
            own_per_split_point: get_dummy_pubkey(),
            counter_per_split_point: get_dummy_pubkey(),
            own_split_adaptor_signature: get_dummy_adaptor_signature(),
            counter_split_adaptor_signature: get_dummy_adaptor_signature(),
            split_tx: SplitTx {
                transaction: get_dummy_tx(),
                output_script: Default::default(),
            },
            ln_glue_transaction: get_dummy_tx(),
            counter_glue_signature: get_dummy_signature(),
            ln_rollback: get_dummy_rollback_info(),
        }
    }

    fn get_dummy_close_offered_sub_channel() -> CloseOfferedSubChannel {
        CloseOfferedSubChannel {
            signed_subchannel: get_dummy_signed_sub_channel(),
            offer_balance: 0,
            accept_balance: 0,
            is_offer: false,
        }
    }

    fn get_dummy_close_accepted_sub_channel() -> CloseAcceptedSubChannel {
        CloseAcceptedSubChannel {
            signed_subchannel: get_dummy_signed_sub_channel(),
            own_balance: 0,
            counter_balance: 0,
            ln_rollback: get_dummy_rollback_info(),
            commitment_transactions: vec![],
        }
    }

    fn get_dummy_offered_sub_channel() -> OfferedSubChannel {
        OfferedSubChannel {
            per_split_point: get_dummy_pubkey(),
        }
    }

    fn get_dummy_accepted_sub_channel() -> AcceptedSubChannel {
        AcceptedSubChannel {
            offer_per_split_point: get_dummy_pubkey(),
            accept_per_split_point: get_dummy_pubkey(),
            split_tx: SplitTx {
                transaction: get_dummy_tx(),
                output_script: Default::default(),
            },
            ln_glue_transaction: get_dummy_tx(),
            ln_rollback: get_dummy_rollback_info(),
            commitment_transactions: vec![],
        }
    }

    fn get_dummy_pubkey() -> PublicKey {
        PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
            .unwrap()
    }

    fn get_dummy_tx() -> Transaction {
        Transaction {
            version: 1,
            lock_time: PackedLockTime::ZERO,
            input: vec![],
            output: vec![],
        }
    }

    fn get_dummy_signature() -> Signature {
        Signature::from_str(
            "304402202f2545f818a5dac9311157d75065156b141e5a6437e817d1d75f9fab084e46940220757bb6f0916f83b2be28877a0d6b05c45463794e3c8c99f799b774443575910d",
        ).unwrap()
    }

    fn get_dummy_adaptor_signature() -> EcdsaAdaptorSignature {
        "03424d14a5471c048ab87b3b83f6085d125d5864249ae4297a57c84e74710bb6730223f325042fce535d040fee52ec13231bf709ccd84233c6944b90317e62528b2527dff9d659a96db4c99f9750168308633c1867b70f3a18fb0f4539a1aecedcd1fc0148fc22f36b6303083ece3f872b18e35d368b3958efe5fb081f7716736ccb598d269aa3084d57e1855e1ea9a45efc10463bbf32ae378029f5763ceb40173f"
            .parse()
            .unwrap()
    }

    fn get_dummy_rollback_info() -> LnRollBackInfo {
        LnRollBackInfo {
            channel_value_satoshis: 0,
            value_to_self_msat: 0,
            funding_outpoint: OutPoint {
                txid: get_dummy_tx().txid(),
                index: 0,
            },
        }
    }
}
