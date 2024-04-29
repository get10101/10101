use crate::dlc::TracingLogger;
use crate::message_handler::TenTenOneMessageHandler;
use crate::networking::DynamicSocketDescriptor;
use dlc_custom_signer::CustomKeysManager;
use lightning::ln::peer_handler::ErroringMessageHandler;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use std::fmt;
use std::sync::Arc;

mod blockchain;
mod dlc_custom_signer;
mod dlc_wallet;
mod fee_rate_estimator;
mod on_chain_wallet;
mod shadow;

pub mod bitcoin_conversion;
pub mod bitmex_client;
pub mod cfd;
pub mod commons;
pub mod config;
pub mod dlc;
pub mod dlc_message;
pub mod message_handler;
pub mod networking;
pub mod node;
pub mod seed;
pub mod storage;
pub mod transaction;

pub use commons::FundingFeeEvent;
pub use config::CONFIRMATION_TARGET;
pub use dlc::ContractDetails;
pub use dlc::DlcChannelDetails;
pub use lightning;
pub use on_chain_wallet::ConfirmationStatus;
pub use on_chain_wallet::FeeConfig;
pub use on_chain_wallet::TransactionDetails;

#[cfg(test)]
mod tests;

pub(crate) type PeerManager<D> = lightning::ln::peer_handler::PeerManager<
    DynamicSocketDescriptor,
    Arc<ErroringMessageHandler>,
    Arc<IgnoringMessageHandler>,
    Arc<TenTenOneMessageHandler>,
    Arc<TracingLogger>,
    Arc<TenTenOneMessageHandler>,
    Arc<CustomKeysManager<D>>,
>;

#[derive(Debug, Clone, Copy)]
pub enum PaymentFlow {
    Inbound,
    Outbound,
}

impl fmt::Display for PaymentFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentFlow::Inbound => "Inbound".fmt(f),
            PaymentFlow::Outbound => "Outbound".fmt(f),
        }
    }
}
