use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::Channel;

pub mod dlc_handler;
mod subscriber;

#[derive(Clone, Debug)]
pub struct DlcChannel {
    pub reference_id: String,
    pub channel_id: String,
    pub channel_state: ChannelState,
}

#[derive(Clone, Debug)]
pub enum ChannelState {
    Offered {
        contract_id: String,
    },
    Accepted {
        contract_id: String,
    },
    Signed {
        contract_id: Option<String>,
        funding_txid: String,
        funding_tx_vout: usize,
        closing_txid: Option<String>,
        state: SignedChannelState,
    },
    Closing {
        contract_id: String,
        buffer_txid: String,
    },
    SettledClosing {
        settle_txid: String,
    },
    Closed {
        closing_txid: String,
    },
    CounterClosed {
        closing_txid: String,
    },
    ClosedPunished,
    CollaborativelyClosed {
        closing_txid: String,
    },
    FailedAccept,
    FailedSign,
    Cancelled {
        contract_id: String,
    },
}

#[derive(Debug, Clone)]
pub enum SignedChannelState {
    Established,
    SettledOffered,
    SettledReceived,
    SettledAccepted,
    SettledConfirmed,
    Settled,
    SettledClosing,
    RenewOffered,
    RenewAccepted,
    RenewConfirmed,
    RenewFinalized,
    Closing,
    CollaborativeCloseOffered,
}

impl From<&Channel> for DlcChannel {
    fn from(value: &Channel) -> Self {
        let channel_state = match value {
            Channel::Offered(o) => ChannelState::Offered {
                contract_id: hex::encode(o.offered_contract_id),
            },
            Channel::Accepted(a) => ChannelState::Accepted {
                contract_id: hex::encode(a.accepted_contract_id),
            },
            s @ Channel::Signed(SignedChannel {
                                    state: dlc_manager::channel::signed_channel::SignedChannelState::CollaborativeCloseOffered {
                                        close_tx,
                                        ..
                                    },
                                    fund_tx,
                                    fund_output_index,
                                    ..
                                }) => ChannelState::Signed {
                contract_id: s.get_contract_id().map(hex::encode),
                funding_txid: fund_tx.txid().to_string(),
                funding_tx_vout: *fund_output_index,
                closing_txid: Some(close_tx.txid().to_string()),
                state: SignedChannelState::CollaborativeCloseOffered,
            },
            s @ Channel::Signed(SignedChannel {
                                    state: dlc_manager::channel::signed_channel::SignedChannelState::SettledClosing {
                                        settle_transaction,
                                        ..
                                    },
                                    fund_tx,
                                    fund_output_index,
                                    ..
                                }) => ChannelState::Signed {
                contract_id: s.get_contract_id().map(hex::encode),
                funding_txid: fund_tx.txid().to_string(),
                funding_tx_vout: *fund_output_index,
                closing_txid: Some(settle_transaction.txid().to_string()),
                state: SignedChannelState::SettledClosing,
            },
            Channel::Signed(s) => ChannelState::Signed {
                contract_id: s.get_contract_id().map(hex::encode),
                funding_txid: s.fund_tx.txid().to_string(),
                funding_tx_vout: s.fund_output_index,
                closing_txid: None,
                state: SignedChannelState::from(&s.state),
            },
            Channel::Closing(c) => ChannelState::Closing {
                buffer_txid: c.buffer_transaction.txid().to_string(),
                contract_id: hex::encode(c.contract_id),
            },
            Channel::SettledClosing(c) => ChannelState::SettledClosing {
                settle_txid: c.claim_transaction.txid().to_string(),
            },
            Channel::Closed(c) => ChannelState::Closed {
                closing_txid: c.closing_txid.to_string()
            },
            Channel::CounterClosed(c) => ChannelState::CounterClosed{
                closing_txid: c.closing_txid.to_string()
            },
            Channel::ClosedPunished(_) => ChannelState::ClosedPunished,
            Channel::CollaborativelyClosed(c) => ChannelState::CollaborativelyClosed{
                closing_txid: c.closing_txid.to_string()
            },
            Channel::FailedAccept(_) => ChannelState::FailedAccept,
            Channel::FailedSign(_) => ChannelState::FailedSign,
            Channel::Cancelled(o) => ChannelState::Cancelled {
                contract_id: hex::encode(o.temporary_channel_id),
            },
        };

        let reference_id = value
            .get_reference_id()
            .map(hex::encode)
            .unwrap_or(hex::encode(value.get_id()));

        Self {
            reference_id,
            channel_id: hex::encode(value.get_id()),
            channel_state,
        }
    }
}

impl From<&dlc_manager::channel::signed_channel::SignedChannelState> for SignedChannelState {
    fn from(value: &dlc_manager::channel::signed_channel::SignedChannelState) -> Self {
        match value {
            dlc_manager::channel::signed_channel::SignedChannelState::Established { .. } => SignedChannelState::Established,
            dlc_manager::channel::signed_channel::SignedChannelState::SettledOffered { .. } => SignedChannelState::SettledOffered,
            dlc_manager::channel::signed_channel::SignedChannelState::SettledReceived { .. } => SignedChannelState::SettledReceived,
            dlc_manager::channel::signed_channel::SignedChannelState::SettledAccepted { .. } => SignedChannelState::SettledAccepted,
            dlc_manager::channel::signed_channel::SignedChannelState::SettledConfirmed { .. } => SignedChannelState::SettledConfirmed,
            dlc_manager::channel::signed_channel::SignedChannelState::Settled { .. } => SignedChannelState::Settled,
            dlc_manager::channel::signed_channel::SignedChannelState::RenewOffered { .. } => SignedChannelState::RenewOffered,
            dlc_manager::channel::signed_channel::SignedChannelState::RenewAccepted { .. } => SignedChannelState::RenewAccepted,
            dlc_manager::channel::signed_channel::SignedChannelState::RenewConfirmed { .. } => SignedChannelState::RenewConfirmed,
            dlc_manager::channel::signed_channel::SignedChannelState::RenewFinalized { .. } => SignedChannelState::RenewFinalized,
            dlc_manager::channel::signed_channel::SignedChannelState::Closing { .. } => SignedChannelState::Closing,
            dlc_manager::channel::signed_channel::SignedChannelState::CollaborativeCloseOffered { .. } => SignedChannelState::CollaborativeCloseOffered,
            dlc_manager::channel::signed_channel::SignedChannelState::SettledClosing { .. } => SignedChannelState::SettledClosing,
        }
    }
}
