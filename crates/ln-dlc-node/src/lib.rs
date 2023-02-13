use crate::logger::TracingLogger;
use bitcoin::secp256k1::PublicKey;
use dlc_manager::custom_signer::CustomKeysManager;
use dlc_manager::custom_signer::CustomSigner;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::Filter;
use lightning::ln::channelmanager::InterceptId;
use lightning::ln::peer_handler::IgnoringMessageHandler;
use lightning::ln::PaymentHash;
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
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

mod disk;
mod ln;
mod ln_dlc_wallet;
mod logger;
mod on_chain_wallet;
mod seed;
mod util;

pub mod node;

#[cfg(test)]
mod tests;

type ChainMonitor = chainmonitor::ChainMonitor<
    CustomSigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<LnDlcWallet>,
    Arc<LnDlcWallet>,
    Arc<TracingLogger>,
    Arc<FilesystemPersister>,
>;

type PeerManager = lightning::ln::peer_handler::PeerManager<
    SocketDescriptor,
    Arc<ChannelManager>,
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

type ChannelManager = lightning::ln::channelmanager::ChannelManager<
    Arc<ChainMonitor>,
    Arc<LnDlcWallet>,
    Arc<CustomKeysManager>,
    Arc<LnDlcWallet>,
    Arc<TracingLogger>,
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
type PaymentInfoStorage = Arc<Mutex<HashMap<PaymentHash, PaymentInfo>>>;
type FakeChannelPaymentRequests = Arc<Mutex<HashMap<RequestedScid, PublicKey>>>;
type PendingInterceptedHtlcs = Arc<Mutex<HashMap<PublicKey, (InterceptId, u64)>>>;

struct PaymentInfo {
    preimage: Option<PaymentPreimage>,
    secret: Option<PaymentSecret>,
    status: HTLCStatus,
    amt_msat: MillisatAmount,
}

enum HTLCStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug)]
struct MillisatAmount(Option<u64>);
