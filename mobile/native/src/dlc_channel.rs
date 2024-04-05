use crate::dlc;
use flutter_rust_bridge::frb;

#[frb]
#[derive(Clone)]
pub struct DlcChannel {
    pub reference_id: String,
    pub dlc_channel_id: String,
    pub channel_state: ChannelState,
}

#[frb]
#[derive(Debug, Clone)]
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

#[frb]
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

impl From<dlc::DlcChannel> for DlcChannel {
    fn from(value: dlc::DlcChannel) -> Self {
        DlcChannel {
            reference_id: value.reference_id,
            dlc_channel_id: value.channel_id,
            channel_state: value.channel_state.into(),
        }
    }
}

impl From<dlc::ChannelState> for ChannelState {
    fn from(value: dlc::ChannelState) -> Self {
        match value {
            dlc::ChannelState::Offered { contract_id } => ChannelState::Offered { contract_id },
            dlc::ChannelState::Accepted { contract_id } => ChannelState::Accepted { contract_id },
            dlc::ChannelState::Signed {
                contract_id,
                funding_txid,
                funding_tx_vout,
                closing_txid,
                state,
            } => ChannelState::Signed {
                contract_id,
                funding_txid,
                funding_tx_vout,
                closing_txid,
                state: SignedChannelState::from(state),
            },
            dlc::ChannelState::Closing {
                contract_id,
                buffer_txid,
            } => ChannelState::Closing {
                contract_id,
                buffer_txid,
            },
            dlc::ChannelState::SettledClosing { settle_txid } => {
                ChannelState::SettledClosing { settle_txid }
            }
            dlc::ChannelState::Closed { closing_txid } => ChannelState::Closed { closing_txid },
            dlc::ChannelState::CounterClosed { closing_txid } => {
                ChannelState::CounterClosed { closing_txid }
            }
            dlc::ChannelState::ClosedPunished => ChannelState::ClosedPunished,
            dlc::ChannelState::CollaborativelyClosed { closing_txid } => {
                ChannelState::CollaborativelyClosed { closing_txid }
            }
            dlc::ChannelState::FailedAccept => ChannelState::FailedAccept,
            dlc::ChannelState::FailedSign => ChannelState::FailedSign,
            dlc::ChannelState::Cancelled { contract_id } => ChannelState::Cancelled { contract_id },
        }
    }
}

impl From<dlc::SignedChannelState> for SignedChannelState {
    fn from(value: dlc::SignedChannelState) -> Self {
        match value {
            dlc::SignedChannelState::Established { .. } => SignedChannelState::Established,
            dlc::SignedChannelState::SettledOffered { .. } => SignedChannelState::SettledOffered,
            dlc::SignedChannelState::SettledReceived { .. } => SignedChannelState::SettledReceived,
            dlc::SignedChannelState::SettledAccepted { .. } => SignedChannelState::SettledAccepted,
            dlc::SignedChannelState::SettledConfirmed { .. } => {
                SignedChannelState::SettledConfirmed
            }
            dlc::SignedChannelState::Settled { .. } => SignedChannelState::Settled,
            dlc::SignedChannelState::RenewOffered { .. } => SignedChannelState::RenewOffered,
            dlc::SignedChannelState::RenewAccepted { .. } => SignedChannelState::RenewAccepted,
            dlc::SignedChannelState::RenewConfirmed { .. } => SignedChannelState::RenewConfirmed,
            dlc::SignedChannelState::RenewFinalized { .. } => SignedChannelState::RenewFinalized,
            dlc::SignedChannelState::Closing { .. } => SignedChannelState::Closing,
            dlc::SignedChannelState::CollaborativeCloseOffered { .. } => {
                SignedChannelState::CollaborativeCloseOffered
            }
            dlc::SignedChannelState::SettledClosing { .. } => SignedChannelState::SettledClosing,
        }
    }
}
