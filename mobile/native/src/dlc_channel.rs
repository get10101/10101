use bitcoin::hashes::hex::ToHex;
use dlc_manager::channel::signed_channel::SignedChannel;
use dlc_manager::channel::Channel;
use flutter_rust_bridge::frb;

#[frb]
pub struct DlcChannel {
    pub dlc_channel_id: String,
    pub channel_state: ChannelState,
}

impl From<&Channel> for DlcChannel {
    fn from(value: &Channel) -> Self {
        let channel_state = match value {
            Channel::Offered(o) => ChannelState::Offered {
                contract_id: o.offered_contract_id.to_hex(),
            },
            Channel::Accepted(a) => ChannelState::Accepted {
                contract_id: a.accepted_contract_id.to_hex(),
            },
            s @ Channel::Signed(SignedChannel {
                state: dlc_manager::channel::signed_channel::SignedChannelState::CollaborativeCloseOffered { close_tx, .. },
                fund_tx,
                fund_output_index,
                ..
            }) => ChannelState::Signed {
                contract_id: s.get_contract_id().map(|c| c.to_hex()),
                funding_txid: fund_tx.txid().to_hex(),
                funding_tx_vout: *fund_output_index,
                closing_txid: Some(close_tx.txid().to_hex()),
                state: SignedChannelState::CollaborativeCloseOffered,
            },
            Channel::Signed(s) => ChannelState::Signed {
                contract_id: s.get_contract_id().map(|c| c.to_hex()),
                funding_txid: s.fund_tx.txid().to_hex(),
                funding_tx_vout: s.fund_output_index,
                closing_txid: None,
                state: SignedChannelState::from(&s.state),
            },
            Channel::Closing(c) => ChannelState::Closing {
                buffer_txid: c.buffer_transaction.txid().to_hex(),
                contract_id: c.contract_id.to_hex(),
            },
            Channel::Closed(c) => ChannelState::Closed{closing_txid: c.closing_txid.to_hex()},
            Channel::CounterClosed(c) => ChannelState::CounterClosed{closing_txid: c.closing_txid.to_hex()},
            Channel::ClosedPunished(_) => ChannelState::ClosedPunished,
            Channel::CollaborativelyClosed(c) => ChannelState::CollaborativelyClosed{closing_txid: c.closing_txid.to_hex()},
            Channel::FailedAccept(_) => ChannelState::FailedAccept,
            Channel::FailedSign(_) => ChannelState::FailedSign,
            Channel::Cancelled(o) => ChannelState::Cancelled {
                contract_id: o.temporary_channel_id.to_hex(),
            },
        };

        Self {
            dlc_channel_id: value.get_id().to_hex(),
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
        }
    }
}

#[frb]
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

#[frb]
pub enum SignedChannelState {
    Established,
    SettledOffered,
    SettledReceived,
    SettledAccepted,
    SettledConfirmed,
    Settled,
    RenewOffered,
    RenewAccepted,
    RenewConfirmed,
    RenewFinalized,
    Closing,
    CollaborativeCloseOffered,
}
