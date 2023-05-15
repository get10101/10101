extern crate core;

use crate::ln::TracingLogger;
use bitcoin::secp256k1::PublicKey;
use dlc_custom_signer::CustomSigner;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::Filter;
use lightning::ln::channelmanager::InterceptId;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::PaymentPreimage;
use lightning::ln::PaymentSecret;
use lightning::routing::gossip;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning_invoice::payment;
use lightning_net_tokio::SocketDescriptor;
use lightning_persister::FilesystemPersister;
use ln_dlc_wallet::LnDlcWallet;
use node::ChannelManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use time::OffsetDateTime;

mod disk;
mod dlc_custom_signer;
mod ldk_node_wallet;
mod ln;
mod ln_dlc_wallet;
mod on_chain_wallet;
mod util;

pub mod node;
pub mod seed;

#[cfg(test)]
mod tests;

use crate::node::SubChannelManager;
pub use ldk_node_wallet::WalletSettings;
pub use ln::ChannelDetails;
pub use ln::DlcChannelDetails;
pub use node::invoice::HTLCStatus;

type ChainMonitor = chainmonitor::ChainMonitor<
    CustomSigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<LnDlcWallet>,
    Arc<LnDlcWallet>,
    Arc<TracingLogger>,
    Arc<FilesystemPersister>,
>;

pub type PeerManager = lightning::ln::peer_handler::PeerManager<
    SocketDescriptor,
    Arc<SubChannelManager>,
    Arc<
        P2PGossipSync<
            Arc<gossip::NetworkGraph<Arc<TracingLogger>>>,
            Arc<dyn chain::Access + Send + Sync>,
            Arc<TracingLogger>,
        >,
    >,
    Arc<IgnoringMessageHandler>,
    Arc<TracingLogger>,
    Arc<DlcMessageHandler>,
>;

pub(crate) type InvoicePayer<E> =
    payment::InvoicePayer<Arc<ChannelManager>, Router, Arc<TracingLogger>, E>;

type Router = DefaultRouter<
    Arc<NetworkGraph>,
    Arc<TracingLogger>,
    Arc<Mutex<ProbabilisticScorer<Arc<NetworkGraph>, Arc<TracingLogger>>>>,
>;

type NetworkGraph = gossip::NetworkGraph<Arc<TracingLogger>>;

type RequestedScid = u64;
type FakeChannelPaymentRequests = Arc<Mutex<HashMap<RequestedScid, PublicKey>>>;
type PendingInterceptedHtlcs = Arc<Mutex<HashMap<PublicKey, (InterceptId, u64)>>>;

#[derive(Clone, Copy)]
pub struct PaymentInfo {
    pub preimage: Option<PaymentPreimage>,
    pub secret: Option<PaymentSecret>,
    pub status: HTLCStatus,
    pub amt_msat: MillisatAmount,
    pub flow: PaymentFlow,
    pub timestamp: OffsetDateTime,
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
