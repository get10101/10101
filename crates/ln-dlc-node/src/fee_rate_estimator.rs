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

pub struct FeeRateEstimator {
    client: esplora_client::AsyncClient,
    fee_rate_cache: RwLock<HashMap<ConfirmationTarget, FeeRate>>,
    fallbacks: RwLock<FeeRateFallbacks>,
}

#[derive(Debug)]
pub struct FeeRateFallbacks {
    pub normal_priority: u32,
    pub high_priority: u32,
}

impl Default for FeeRateFallbacks {
    fn default() -> Self {
        Self {
            normal_priority: 2000,
            high_priority: 5000,
        }
    }
}

impl FeeRateEstimator {
    pub fn new(esplora_url: String) -> Self {
        let client = esplora_client::AsyncClient::from_client(esplora_url, reqwest::Client::new());

        let fee_rate_cache = RwLock::new(HashMap::default());
        let fallbacks = RwLock::new(FeeRateFallbacks::default());

        Self {
            client,
            fee_rate_cache,
            fallbacks,
        }
    }

    pub(crate) fn get(&self, target: ConfirmationTarget) -> FeeRate {
        self.cache_read_lock()
            .get(&target)
            .copied()
            .unwrap_or_else(|| {
                let fee_rate = match target {
                    ConfirmationTarget::Background => FEERATE_FLOOR_SATS_PER_KW,
                    ConfirmationTarget::Normal => self.fallbacks_read_lock().normal_priority,
                    ConfirmationTarget::HighPriority => self.fallbacks_read_lock().high_priority,
                };
                FeeRate::from_sat_per_kwu(fee_rate as f32)
            })
    }

    #[autometrics]
    pub(crate) async fn update(&self) -> Result<()> {
        let estimates = self.client.get_fee_estimates().await?;

        let mut locked_fee_rate_cache = self.cache_write_lock();
        for (target, n_blocks) in [
            (ConfirmationTarget::Background, 12),
            (ConfirmationTarget::Normal, 6),
            (ConfirmationTarget::HighPriority, 3),
        ] {
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

    pub fn update_fallbacks(&self, fallbacks: FeeRateFallbacks) {
        tracing::info!(?fallbacks, "Updating fee rate fallbacks");
        *self.fallbacks_write_lock() = fallbacks;
    }

    fn fallbacks_read_lock(&self) -> RwLockReadGuard<FeeRateFallbacks> {
        self.fallbacks.read().expect("RwLock to not be poisoned")
    }

    fn fallbacks_write_lock(&self) -> RwLockWriteGuard<FeeRateFallbacks> {
        self.fallbacks.write().expect("RwLock to not be poisoned")
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
        (self.get(confirmation_target).fee_wu(1000) as u32).max(FEERATE_FLOOR_SATS_PER_KW)
    }
}
