use crate::blockchain::Blockchain;
use crate::ln::TracingLogger;
use crate::node::SubChannelManager;
use crate::node::TenTenOneOnionMessageHandler;
use dlc_custom_signer::CustomKeysManager;
use dlc_custom_signer::CustomSigner;
use dlc_messages::message_handler::MessageHandler as DlcMessageHandler;
use fee_rate_estimator::FeeRateEstimator;
use lightning::chain::chainmonitor;
use lightning::chain::Filter;
use lightning::routing::gossip;
use lightning::routing::gossip::P2PGossipSync;
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::ProbabilisticScorer;
use lightning::routing::scoring::ProbabilisticScoringFeeParameters;
use lightning::routing::utxo::UtxoLookup;
use std::fmt;
use std::sync::Arc;

mod blockchain;
mod dlc_custom_signer;
mod dlc_wallet;
mod fee_rate_estimator;
mod on_chain_wallet;
mod shadow;

pub mod bitcoin_conversion;
pub mod config;
pub mod dlc_message;
pub mod ln;
pub mod networking;
pub mod node;
pub mod seed;
pub mod storage;
pub mod transaction;

use crate::networking::DynamicSocketDescriptor;
pub use config::CONFIRMATION_TARGET;
pub use lightning;
pub use lightning_invoice;
pub use ln::AppEventHandler;
pub use ln::ContractDetails;
pub use ln::CoordinatorEventHandler;
pub use ln::DlcChannelDetails;
pub use ln::EventHandlerTrait;
pub use ln::EventSender;
pub use on_chain_wallet::ConfirmationStatus;
pub use on_chain_wallet::EstimateFeeError;
pub use on_chain_wallet::TransactionDetails;

#[cfg(test)]
mod tests;

type ChainMonitor<S, N> = chainmonitor::ChainMonitor<
    CustomSigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<Blockchain<N>>,
    Arc<FeeRateEstimator>,
    Arc<TracingLogger>,
    Arc<S>,
>;

pub type PeerManager<D, S, N> = lightning::ln::peer_handler::PeerManager<
    DynamicSocketDescriptor,
    Arc<SubChannelManager<D, S, N>>,
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
    Arc<CustomKeysManager<D>>,
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

type P2pGossipSync =
    P2PGossipSync<Arc<NetworkGraph>, Arc<dyn UtxoLookup + Send + Sync>, Arc<TracingLogger>>;

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
