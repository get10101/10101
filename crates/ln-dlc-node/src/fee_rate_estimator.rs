use anyhow::Result;
use bdk::FeeRate;
use bitcoin::Network;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::chain::chaininterface::FEERATE_FLOOR_SATS_PER_KW;
use parking_lot::RwLock;
use std::collections::HashMap;

/// Default values used when constructing the [`FeeRateEstimator`] if the fee rate sever cannot give
/// us up-to-date values.
///
/// In sats/kwu.
const FEE_RATE_DEFAULTS: [(ConfirmationTarget, u32); 4] = [
    (ConfirmationTarget::MempoolMinimum, 1000),
    (ConfirmationTarget::Background, 2000),
    (ConfirmationTarget::Normal, 3000),
    (ConfirmationTarget::HighPriority, 4000),
];

pub struct FeeRateEstimator {
    client: mempool::MempoolFeeRateEstimator,
    fee_rate_cache: RwLock<HashMap<ConfirmationTarget, FeeRate>>,
}

pub trait EstimateFeeRate {
    fn estimate(&self, target: ConfirmationTarget) -> FeeRate;
}

impl EstimateFeeRate for FeeRateEstimator {
    fn estimate(&self, target: ConfirmationTarget) -> FeeRate {
        self.get(target)
    }
}

fn to_mempool_network(value: Network) -> mempool::Network {
    match value {
        Network::Bitcoin => mempool::Network::Mainnet,
        Network::Testnet => mempool::Network::Testnet,
        Network::Signet => mempool::Network::Signet,
        Network::Regtest => mempool::Network::Local,
    }
}

impl FeeRateEstimator {
    /// Constructor for the [`FeeRateEstimator`].
    pub fn new(network: Network) -> Self {
        let client = mempool::MempoolFeeRateEstimator::new(to_mempool_network(network));

        tracing::warn!(defaults = ?FEE_RATE_DEFAULTS, "Initializing fee rate cache with default values.");

        let initial_fee_rates = HashMap::from_iter(
            FEE_RATE_DEFAULTS
                .into_iter()
                .map(|(target, fee_rate)| (target, FeeRate::from_sat_per_kwu(fee_rate as f32))),
        );

        let fee_rate_cache = RwLock::new(initial_fee_rates);

        Self {
            client,
            fee_rate_cache,
        }
    }

    pub fn get(&self, target: ConfirmationTarget) -> FeeRate {
        self.fee_rate_cache
            .read()
            .get(&target)
            .copied()
            .expect("to have entries for all confirmation targets")
    }

    pub(crate) async fn update(&self) -> Result<()> {
        let estimates = self.client.fetch_fee().await?;

        let mut locked_fee_rate_cache = self.fee_rate_cache.write();

        locked_fee_rate_cache.insert(
            ConfirmationTarget::MempoolMinimum,
            FeeRate::from_sat_per_vb(estimates.minimum_fee as f32),
        );

        locked_fee_rate_cache.insert(
            ConfirmationTarget::Background,
            FeeRate::from_sat_per_vb(estimates.economy_fee as f32),
        );

        locked_fee_rate_cache.insert(
            ConfirmationTarget::Normal,
            FeeRate::from_sat_per_vb(estimates.hour_fee as f32),
        );

        locked_fee_rate_cache.insert(
            ConfirmationTarget::HighPriority,
            FeeRate::from_sat_per_vb(estimates.fastest_fee as f32),
        );

        Ok(())
    }
}

impl FeeEstimator for FeeRateEstimator {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        (self.estimate(confirmation_target).fee_wu(1000) as u32).max(FEERATE_FLOOR_SATS_PER_KW)
    }
}
