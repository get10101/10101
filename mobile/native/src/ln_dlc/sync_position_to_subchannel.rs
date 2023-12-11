use crate::db;
use crate::event;
use crate::event::BackgroundTask;
use crate::event::EventInternal;
use crate::event::TaskStatus;
use crate::ln_dlc::node::Node;
use crate::trade::order;
use crate::trade::position;
use anyhow::Result;
use commons::order_matching_fee_taker;
use lightning::ln::ChannelId;
use ln_dlc_node::node::rust_dlc_manager::Storage;
use rust_decimal::Decimal;
use std::time::Duration;

#[derive(PartialEq, Clone, Debug)]
enum Action {
    ContinueSubchannelProtocol,
    CreatePosition(ChannelId),
    RemovePosition,
}

impl Node {
    /// Sync the position with the subchannel state.
    ///
    /// TODO(holzeis): With https://github.com/get10101/10101/issues/530 we should not require this
    /// logic anymore.
    ///
    /// - [`SubChannelState::Signed`] but no position: create position from `Filling` order.
    ///
    /// - [`SubChannelState::OffChainClosed`] and a position exists: delete the position.
    ///
    /// - [`SubChannelState::CloseOffered`] or [`SubChannelState::CloseAccepted`]: inform the UI
    /// that the subchannel is being recovered.
    ///
    /// - [`SubChannelState::Offered`], [`SubChannelState::Accepted`] or
    ///   [`SubChannelState::Finalized`]: inform the UI that the subchannel is being recovered.
    ///
    /// - Subchannel in any other state, with position: delete position because the channel might
    /// have been force-closed.
    pub async fn sync_position_with_subchannel_state(&self) -> Result<()> {
        let channels = self.inner.channel_manager.list_channels();

        let positions = db::get_positions()?;
        let first_position = positions.first();

        let channel_details = match channels.first() {
            Some(channel_details) => channel_details,
            None => {
                // If we don't have a channel but we do have a position, we can safely close said
                // position.
                if first_position.is_some() {
                    close_position_with_order()?;
                }
                return Ok(());
            }
        };

        let subchannels = self.inner.dlc_manager.get_store().get_sub_channels()?;
        let subchannel = subchannels
            .iter()
            .find(|subchannel| subchannel.channel_id == channel_details.channel_id);

        let position = first_position.map(Position::from);
        let subchannel = subchannel.map(SubChannel::from);

        match determine_sync_position_to_subchannel_action(position, subchannel) {
            Some(Action::ContinueSubchannelProtocol) => self.recover_subchannel().await?,
            Some(Action::CreatePosition(channel_id)) => match order::handler::order_filled() {
                Ok(order) => {
                    let execution_price = order
                        .execution_price()
                        .expect("filled order to have a price");
                    let open_position_fee = order_matching_fee_taker(
                        order.quantity,
                        Decimal::try_from(execution_price)?,
                    );

                    let (accept_collateral, expiry_timestamp) = self
                        .inner
                        .get_collateral_and_expiry_for_confirmed_contract(channel_id)?;

                    position::handler::update_position_after_dlc_creation(
                        order,
                        accept_collateral - open_position_fee.to_sat(),
                        expiry_timestamp,
                    )?;

                    tracing::info!("Successfully recovered position from order");
                }
                Err(e) => {
                    tracing::error!(
                        "Could not recover position from order as no filling order was found. \
                         Error: {e:#}"
                    );
                }
            },
            Some(Action::RemovePosition) => {
                close_position_with_order()?;
            }
            None => (),
        }

        Ok(())
    }

    /// Sends a [`BackgroundNotification::RecoverDlc`] to the UI, so that the UI can convey that the
    /// subchannel protocol is still in progress. Also triggers the `periodic_check` on the
    /// `SubChannelManager` to process any relevant actions that might have been created after the
    /// channel reestablishment.
    ///
    /// FIXME(holzeis): We currently use different events to inform about the recovery of a
    /// subchannel and to inform about the wait for an order execution in the happy case (without a
    /// restart in between). Those events and dialogs should be aligned.
    async fn recover_subchannel(&self) -> Result<()> {
        tracing::warn!("App probably closed whilst executing subchannel protocol");
        event::publish(&EventInternal::BackgroundNotification(
            BackgroundTask::RecoverDlc(TaskStatus::Pending),
        ));

        // HACK(holzeis): We are manually calling the periodic check here to speed up the
        // processing of pending actions.
        //
        // Note: this might not speed up the process, as the coordinator might have to first resend
        // a message to continue the protocol. This should be fixed in `rust-dlc` and any pending
        // actions should be processed immediately once the channel is ready instead of periodically
        // checking if a pending action needs to be processed.
        //
        // Note: pending actions can only get created on channel reestablishment, hence we are
        // waiting for an arbitrary 5 seconds here to increase the likelihood that the channel has
        // been reestablished.
        tokio::time::sleep(Duration::from_secs(5)).await;
        if let Err(e) = self.inner.sub_channel_manager_periodic_check().await {
            tracing::error!("Failed to process periodic check! Error: {e:#}");
        }

        Ok(())
    }
}

/// Determine what needs to be done, if anything, to keep the [`Position`] and [`SubChannel`] in
/// sync.
///
/// ### Returns
///
/// - [`Action::ContinueSubchannelProtocol`] if the subchannel is in an intermediate state.
///
/// - [`Action::CreatePosition`] if the subchannel is in [`SubChannelState::Signed`], but no
/// position exists.
///
/// - [`Action::RemovePosition`] if a position exists and the subchannel is _not_ in
/// [`SubChannelState::Signed`] or an intermediate state.
///
/// - `None` otherwise.
fn determine_sync_position_to_subchannel_action(
    position: Option<Position>,
    subchannel: Option<SubChannel>,
) -> Option<Action> {
    use SubChannel::*;

    tracing::debug!(
        ?position,
        ?subchannel,
        "Checking if position and subchannel state are out of sync"
    );

    if let Some(Position::Resizing) = position {
        let action = match subchannel {
            // Ongoing resize protocol.
            Some(Offered) | Some(Accepted) | Some(Confirmed) | Some(Finalized)
            | Some(CloseOffered) | Some(CloseAccepted) | Some(CloseConfirmed)
            | Some(OffChainClosed) => {
                tracing::debug!("Letting subchannel resize protocol continue");
                Some(Action::ContinueSubchannelProtocol)
            }
            // Closed on-chain.
            Some(Closing)
            | Some(OnChainClosed)
            | Some(CounterOnChainClosed)
            | Some(ClosedPunished) => {
                tracing::warn!("Deleting resizing position as subchannel is closed on-chain");
                Some(Action::RemovePosition)
            }
            // This is _not_ the same as having an `OffChainClosed` subchannel.
            None => {
                tracing::warn!("Deleting resizing position as subchannel does not exist");
                Some(Action::RemovePosition)
            }
            // The offer to reopen the subchannel was rejected. It's probably best to remove the
            // position.
            Some(Rejected) => {
                tracing::warn!("Deleting resizing position as subchannel reopen was rejected");
                Some(Action::RemovePosition)
            }
            // This is weird.
            //
            // TODO: Consider setting the `Position` back to `Open`.
            Some(Signed { .. }) => {
                tracing::warn!("Subchannel is signed but position is resizing");
                None
            }
        };

        return action;
    }

    match (position, subchannel) {
        (Some(_), Some(subchannel)) => {
            match subchannel {
                Signed { .. } => {
                    tracing::debug!("Subchannel and position are in sync");
                    None
                }
                CloseOffered | CloseAccepted => {
                    tracing::debug!("Letting subchannel close protocol continue");
                    Some(Action::ContinueSubchannelProtocol)
                }
                OffChainClosed => {
                    tracing::warn!("Deleting position as subchannel is already closed");
                    Some(Action::RemovePosition)
                }
                Offered | Accepted | Confirmed | Finalized | Closing | OnChainClosed
                | CounterOnChainClosed | CloseConfirmed | ClosedPunished | Rejected => {
                    tracing::warn!(
                        "The subchannel is in a state that cannot be recovered. Removing position"
                    );
                    // Maybe a leftover after a force-closure.
                    Some(Action::RemovePosition)
                }
            }
        }
        (None, Some(subchannel)) => match subchannel {
            OffChainClosed => {
                tracing::debug!("Subchannel and position are in sync");
                None
            }
            Signed { channel_id } => {
                tracing::warn!("Trying to recover position from order");
                Some(Action::CreatePosition(channel_id))
            }
            Offered | Accepted | Finalized => {
                tracing::debug!("Letting subchannel open protocol continue");
                Some(Action::ContinueSubchannelProtocol)
            }
            Confirmed | Closing | OnChainClosed | CounterOnChainClosed | CloseOffered
            | CloseAccepted | CloseConfirmed | ClosedPunished | Rejected => {
                tracing::warn!("The subchannel is in a state that cannot be recovered");
                None
            }
        },
        (Some(_), None) => {
            tracing::warn!("Found position without subchannel. Removing position");
            Some(Action::RemovePosition)
        }
        _ => None,
    }
}

fn close_position_with_order() -> Result<()> {
    let filled_order = order::handler::order_filled().ok();
    position::handler::update_position_after_dlc_closure(filled_order)?;

    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Position {
    Open,
    Closing,
    Rollover,
    Resizing,
}

#[derive(Clone, Copy, Debug)]
enum SubChannel {
    Offered,
    Accepted,
    Confirmed,
    Finalized,
    Signed { channel_id: ChannelId },
    Closing,
    OnChainClosed,
    CounterOnChainClosed,
    CloseOffered,
    CloseAccepted,
    CloseConfirmed,
    OffChainClosed,
    ClosedPunished,
    Rejected,
}

impl From<&position::Position> for Position {
    fn from(value: &position::Position) -> Self {
        match value.position_state {
            position::PositionState::Open => Self::Open,
            position::PositionState::Closing => Self::Closing,
            position::PositionState::Rollover => Self::Rollover,
            position::PositionState::Resizing => Self::Resizing,
        }
    }
}

impl From<&crate::ln_dlc::SubChannel> for SubChannel {
    fn from(value: &crate::ln_dlc::SubChannel) -> Self {
        match value.state {
            crate::ln_dlc::SubChannelState::Offered(_) => Self::Offered,
            crate::ln_dlc::SubChannelState::Accepted(_) => Self::Accepted,
            crate::ln_dlc::SubChannelState::Confirmed(_) => Self::Confirmed,
            crate::ln_dlc::SubChannelState::Finalized(_) => Self::Finalized,
            crate::ln_dlc::SubChannelState::Signed(_) => Self::Signed {
                channel_id: value.channel_id,
            },
            crate::ln_dlc::SubChannelState::Closing(_) => Self::Closing,
            crate::ln_dlc::SubChannelState::OnChainClosed => Self::OnChainClosed,
            crate::ln_dlc::SubChannelState::CounterOnChainClosed => Self::CounterOnChainClosed,
            crate::ln_dlc::SubChannelState::CloseOffered(_) => Self::CloseOffered,
            crate::ln_dlc::SubChannelState::CloseAccepted(_) => Self::CloseAccepted,
            crate::ln_dlc::SubChannelState::CloseConfirmed(_) => Self::CloseConfirmed,
            crate::ln_dlc::SubChannelState::OffChainClosed => Self::OffChainClosed,
            crate::ln_dlc::SubChannelState::ClosedPunished(_) => Self::ClosedPunished,
            crate::ln_dlc::SubChannelState::Rejected => Self::Rejected,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_none_position_and_none_subchannel() {
        let action = determine_sync_position_to_subchannel_action(None, None);
        assert!(action.is_none());
    }

    #[test]
    fn test_some_position_and_none_subchannel() {
        let action = determine_sync_position_to_subchannel_action(Some(Position::Open), None);
        assert_eq!(Some(Action::RemovePosition), action);
    }

    #[test]
    fn test_some_position_and_offchainclosed_subchannel() {
        let action = determine_sync_position_to_subchannel_action(Some(Position::Open), None);
        assert_eq!(Some(Action::RemovePosition), action);
    }

    #[test]
    fn test_some_position_and_signed_subchannel() {
        let action = determine_sync_position_to_subchannel_action(
            Some(Position::Open),
            Some(SubChannel::Signed {
                channel_id: dummy_channel_id(),
            }),
        );
        assert!(action.is_none());
    }

    #[test]
    fn test_some_position_and_close_offered_subchannel() {
        let action = determine_sync_position_to_subchannel_action(
            Some(Position::Open),
            Some(SubChannel::CloseOffered),
        );
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);
    }

    #[test]
    fn test_some_position_and_close_accepted_subchannel() {
        let action = determine_sync_position_to_subchannel_action(
            Some(Position::Open),
            Some(SubChannel::CloseAccepted),
        );
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);
    }

    #[test]
    fn test_none_position_and_offchainclosed_subchannel() {
        let action =
            determine_sync_position_to_subchannel_action(None, Some(SubChannel::OffChainClosed));
        assert!(action.is_none());
    }

    #[test]
    fn test_none_position_and_signed_subchannel() {
        let action = determine_sync_position_to_subchannel_action(
            None,
            Some(SubChannel::Signed {
                channel_id: dummy_channel_id(),
            }),
        );
        assert!(matches!(action, Some(Action::CreatePosition(_))));
    }

    #[test]
    fn test_none_position_and_offered_subchannel() {
        let action = determine_sync_position_to_subchannel_action(None, Some(SubChannel::Offered));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);
    }

    #[test]
    fn test_none_position_and_accepted_subchannel() {
        let action = determine_sync_position_to_subchannel_action(None, Some(SubChannel::Accepted));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);
    }

    #[test]
    fn test_none_position_and_finalized_subchannel() {
        let action =
            determine_sync_position_to_subchannel_action(None, Some(SubChannel::Finalized));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);
    }

    #[test]
    fn test_none_position_and_other_subchannel_state() {
        let action =
            determine_sync_position_to_subchannel_action(None, Some(SubChannel::OnChainClosed));
        assert!(action.is_none());
    }

    #[test]
    fn test_some_position_and_other_subchannel_state() {
        let action = determine_sync_position_to_subchannel_action(
            Some(Position::Open),
            Some(SubChannel::OnChainClosed),
        );
        assert_eq!(Some(Action::RemovePosition), action);
    }

    #[test]
    fn test_resizing_position() {
        use SubChannel::*;

        let position = Some(Position::Resizing);

        let action = determine_sync_position_to_subchannel_action(position, Some(Offered));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(Accepted));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(Confirmed));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(Finalized));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(CloseOffered));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(CloseAccepted));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(CloseConfirmed));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(OffChainClosed));
        assert_eq!(Some(Action::ContinueSubchannelProtocol), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(Closing));
        assert_eq!(Some(Action::RemovePosition), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(OnChainClosed));
        assert_eq!(Some(Action::RemovePosition), action);

        let action =
            determine_sync_position_to_subchannel_action(position, Some(CounterOnChainClosed));
        assert_eq!(Some(Action::RemovePosition), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(ClosedPunished));
        assert_eq!(Some(Action::RemovePosition), action);

        let action = determine_sync_position_to_subchannel_action(position, None);
        assert_eq!(Some(Action::RemovePosition), action);

        let action = determine_sync_position_to_subchannel_action(position, Some(Rejected));
        assert_eq!(Some(Action::RemovePosition), action);

        let action = determine_sync_position_to_subchannel_action(
            position,
            Some(Signed {
                channel_id: dummy_channel_id(),
            }),
        );
        assert!(action.is_none());
    }

    fn dummy_channel_id() -> ChannelId {
        ChannelId([0; 32])
    }
}
