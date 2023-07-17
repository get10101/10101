use crate::ln::TracingLogger;
use crate::node::SubChannelManager;
use bitcoin::hashes::hex::ToHex;
use bitcoin::secp256k1::PublicKey;
use dlc_custom_signer::CustomKeysManager;
use dlc_custom_signer::CustomSigner;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use fee_rate_estimator::FeeRateEstimator;
use lightning::chain::chainmonitor;
use lightning::chain::Filter;
use lightning::ln::channelmanager::InterceptId;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use lightning::routing::gossip;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::routing::utxo::UtxoLookup;
use lightning_invoice::Invoice;
use lightning_invoice::InvoiceDescription;
use lightning_net_tokio::SocketDescriptor;
use lightning_persister::FilesystemPersister;
use ln_dlc_wallet::LnDlcWallet;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use time::OffsetDateTime;

mod disk;
mod dlc_custom_signer;
mod fee_rate_estimator;
mod ldk_node_wallet;
mod ln;
mod ln_dlc_wallet;
mod on_chain_wallet;
mod util;

pub mod node;
pub mod seed;

pub use ln::CONFIRMATION_TARGET;
pub use ln::CONTRACT_TX_FEE_RATE;
pub use ln::JUST_IN_TIME_CHANNEL_OUTBOUND_LIQUIDITY_SAT_MAX;
pub use ln::LIQUIDITY_MULTIPLIER;

#[cfg(test)]
mod tests;

pub use ldk_node_wallet::WalletSettings;
pub use ln::ChannelDetails;
pub use ln::DlcChannelDetails;
pub use node::invoice::HTLCStatus;

type ChainMonitor = chainmonitor::ChainMonitor<
    CustomSigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<LnDlcWallet>,
    Arc<FeeRateEstimator>,
    Arc<TracingLogger>,
    Arc<FilesystemPersister>,
>;

pub type PeerManager = lightning::ln::peer_handler::PeerManager<
    SocketDescriptor,
    Arc<SubChannelManager>,
    Arc<
        P2PGossipSync<
            Arc<gossip::NetworkGraph<Arc<TracingLogger>>>,
            Arc<dyn UtxoLookup + Send + Sync>,
            Arc<TracingLogger>,
        >,
    >,
    Arc<DlcMessageHandler>,
    Arc<TracingLogger>,
    Arc<DlcMessageHandler>,
    Arc<CustomKeysManager>,
>;

pub(crate) type Router = DefaultRouter<
    Arc<NetworkGraph>,
    Arc<TracingLogger>,
    Arc<Mutex<ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>>>>,
>;

type NetworkGraph = gossip::NetworkGraph<Arc<TracingLogger>>;

type RequestedScid = u64;
type FakeChannelPaymentRequests = Arc<Mutex<HashMap<RequestedScid, PublicKey>>>;
type PendingInterceptedHtlcs = Arc<Mutex<HashMap<PublicKey, (InterceptId, u64)>>>;

#[derive(Debug, Clone)]
pub struct PaymentInfo {
    pub preimage: Option<PaymentPreimage>,
    pub secret: Option<PaymentSecret>,
    pub status: HTLCStatus,
    pub amt_msat: MillisatAmount,
    pub flow: PaymentFlow,
    pub timestamp: OffsetDateTime,
    pub description: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PaymentFlow {
    Inbound,
    Outbound,
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

impl From<Invoice> for PaymentInfo {
    fn from(value: Invoice) -> Self {
        Self {
            preimage: None,
            secret: Some(*value.payment_secret()),
            status: HTLCStatus::Pending,
            amt_msat: MillisatAmount(value.amount_milli_satoshis()),
            flow: PaymentFlow::Inbound,
            timestamp: OffsetDateTime::from(value.timestamp()),
            description: match value.description() {
                InvoiceDescription::Direct(direct) => direct.to_string(),
                InvoiceDescription::Hash(hash) => hash.0.to_hex(),
            },
        }
    }
}
