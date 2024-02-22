use crate::ln::TracingLogger;
use crate::node::SubChannelManager;
use crate::node::TenTenOneOnionMessageHandler;
use bitcoin::hashes::hex::ToHex;
use bitcoin::Txid;
use dlc_custom_signer::CustomKeysManager;
use dlc_custom_signer::CustomSigner;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use fee_rate_estimator::FeeRateEstimator;
use lightning::chain::chainmonitor;
use lightning::chain::Filter;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use lightning::routing::gossip;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::routing::scoring::ProbabilisticScoringFeeParameters;
use lightning::routing::utxo::UtxoLookup;
use lightning_invoice::Bolt11Invoice;
use lightning_invoice::Bolt11InvoiceDescription;
use lightning_net_tokio::SocketDescriptor;
use ln_dlc_wallet::LnDlcWallet;
use std::fmt;
use std::sync::Arc;
use time::OffsetDateTime;

mod dlc_custom_signer;
mod fee_rate_estimator;
mod ldk_node_wallet;
mod ln_dlc_wallet;
mod on_chain_wallet;
mod shadow;

pub mod channel;
pub mod config;
pub mod dlc_message;
pub mod ln;
pub mod node;
pub mod scorer;
pub mod seed;
pub mod storage;
pub mod transaction;
pub mod util;

pub use config::CONFIRMATION_TARGET;
pub use ldk_node_wallet::WalletSettings;
pub use lightning;
pub use lightning_invoice;
pub use ln::AppEventHandler;
pub use ln::ChannelDetails;
pub use ln::ContractDetails;
pub use ln::CoordinatorEventHandler;
pub use ln::DlcChannelDetails;
pub use ln::EventHandlerTrait;
pub use ln::EventSender;
pub use node::invoice::HTLCStatus;

#[cfg(test)]
mod tests;

type ChainMonitor<S, N> = chainmonitor::ChainMonitor<
    CustomSigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<LnDlcWallet<S, N>>,
    Arc<FeeRateEstimator>,
    Arc<TracingLogger>,
    Arc<S>,
>;

pub type PeerManager<S, N> = lightning::ln::peer_handler::PeerManager<
    SocketDescriptor,
    Arc<SubChannelManager<S, N>>,
    Arc<
        P2PGossipSync<
            Arc<gossip::NetworkGraph<Arc<TracingLogger>>>,
            Arc<dyn UtxoLookup + Send + Sync>,
            Arc<TracingLogger>,
        >,
    >,
    Arc<TenTenOneOnionMessageHandler>,
    Arc<TracingLogger>,
    Arc<DlcMessageHandler>,
    Arc<CustomKeysManager<S, N>>,
>;

pub(crate) type Router = DefaultRouter<
    Arc<NetworkGraph>,
    Arc<TracingLogger>,
    Arc<std::sync::RwLock<Scorer>>,
    ProbabilisticScoringFeeParameters,
    Scorer,
>;
pub(crate) type Scorer = ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>>;

type NetworkGraph = gossip::NetworkGraph<Arc<TracingLogger>>;

#[derive(Debug, Clone)]
pub struct PaymentInfo {
    pub preimage: Option<PaymentPreimage>,
    pub secret: Option<PaymentSecret>,
    pub status: HTLCStatus,
    pub amt_msat: MillisatAmount,
    pub fee_msat: MillisatAmount,
    pub flow: PaymentFlow,
    pub timestamp: OffsetDateTime,
    pub description: String,
    pub invoice: Option<String>,
    /// If the payment was used to open an inbound channel, this tx id refers the funding
    /// transaction for opening the channel.
    pub funding_txid: Option<Txid>,
}

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

#[derive(Debug, Clone, Copy)]
pub struct MillisatAmount(Option<u64>);

impl MillisatAmount {
    pub fn new(amount: Option<u64>) -> Self {
        Self(amount)
    }

    pub fn to_inner(&self) -> Option<u64> {
        self.0
    }
}

impl From<Bolt11Invoice> for PaymentInfo {
    fn from(value: Bolt11Invoice) -> Self {
        Self {
            preimage: None,
            secret: Some(*value.payment_secret()),
            status: HTLCStatus::Pending,
            amt_msat: MillisatAmount(value.amount_milli_satoshis()),
            fee_msat: MillisatAmount(None),
            flow: PaymentFlow::Inbound,
            timestamp: OffsetDateTime::from(value.timestamp()),
            description: match value.description() {
                Bolt11InvoiceDescription::Direct(direct) => direct.to_string(),
                Bolt11InvoiceDescription::Hash(hash) => hash.0.to_hex(),
            },
            invoice: Some(value.to_string()),
            funding_txid: None,
        }
    }
}
