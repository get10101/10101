use anyhow::Result;
use autometrics::autometrics;
use bdk::FeeRate;
use lightning::chain::chaininterface::ConfirmationTarget;
use lightning::chain::chaininterface::FeeEstimator;
use lightning::chain::chaininterface::FEERATE_FLOOR_SATS_PER_KW;
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;

const CONFIRMATION_TARGETS: [(ConfirmationTarget, usize); 3] = [
    // We choose an extremely high background confirmation target to avoid force-closing channels
    // unnecessarily.
    (ConfirmationTarget::Background, 1008),
    (ConfirmationTarget::Normal, 6),
    (ConfirmationTarget::HighPriority, 3),
];

/// Default values used when constructing the [`FeeRateEstimator`] if the fee rate sever cannot give
/// us up-to-date values.
///
/// In sats/kwu.
const FEE_RATE_DEFAULTS: [(ConfirmationTarget, u32); 3] = [
    (ConfirmationTarget::Background, FEERATE_FLOOR_SATS_PER_KW),
    (ConfirmationTarget::Normal, 2000),
    (ConfirmationTarget::HighPriority, 5000),
];

pub struct FeeRateEstimator {
    client: esplora_client::BlockingClient,
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

impl FeeRateEstimator {
    /// Constructor for the [`FeeRateEstimator`].
    pub fn new(esplora_url: String) -> Self {
        let client = esplora_client::BlockingClient::from_agent(esplora_url, ureq::agent());

        let initial_fee_rates = match client.get_fee_estimates() {
            Ok(estimates) => {
                HashMap::from_iter(CONFIRMATION_TARGETS.into_iter().map(|(target, n_blocks)| {
                    let fee_rate = esplora_client::convert_fee_rate(n_blocks, estimates.clone())
                        .expect("fee rates for our confirmation targets");
                    let fee_rate = FeeRate::from_sat_per_vb(fee_rate);

                    (target, fee_rate)
                }))
            }
            Err(e) => {
                tracing::warn!(defaults = ?FEE_RATE_DEFAULTS, "Initializing fee rate cache with default values: {e:#}");

                HashMap::from_iter(
                    FEE_RATE_DEFAULTS.into_iter().map(|(target, fee_rate)| {
                        (target, FeeRate::from_sat_per_kwu(fee_rate as f32))
                    }),
                )
            }
        };

        let fee_rate_cache = RwLock::new(initial_fee_rates);

        Self {
            client,
            fee_rate_cache,
        }
    }

    fn get(&self, target: ConfirmationTarget) -> FeeRate {
        self.cache_read_lock()
            .get(&target)
            .copied()
            .expect("to have entries for all confirmation targets")
    }

    #[autometrics]
    pub(crate) async fn update(&self) -> Result<()> {
        let estimates = self.client.get_fee_estimates()?;

        let mut locked_fee_rate_cache = self.cache_write_lock();
        for (target, n_blocks) in CONFIRMATION_TARGETS {
            let fee_rate = esplora_client::convert_fee_rate(n_blocks, estimates.clone())?;

            let fee_rate = FeeRate::from_sat_per_vb(fee_rate);

            locked_fee_rate_cache.insert(target, fee_rate);
            tracing::trace!(
                n_blocks_confirmation = %n_blocks,
                sats_per_kwu = %fee_rate.fee_wu(1000),
                "Updated fee rate estimate",
            );
        }

        Ok(())
    }

    fn cache_read_lock(&self) -> RwLockReadGuard<HashMap<ConfirmationTarget, FeeRate>> {
        self.fee_rate_cache
            .read()
            .expect("RwLock to not be poisoned")
    }

    fn cache_write_lock(&self) -> RwLockWriteGuard<HashMap<ConfirmationTarget, FeeRate>> {
        self.fee_rate_cache
            .write()
            .expect("RwLock to not be poisoned")
    }
}

impl FeeEstimator for FeeRateEstimator {
    #[autometrics]
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        (self.estimate(confirmation_target).fee_wu(1000) as u32).max(FEERATE_FLOOR_SATS_PER_KW)
    }
}
