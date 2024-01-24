use anyhow::Result;
use serde::Deserialize;

const MEMPOOL_FEE_RATE_URL_MAINNET: &str = "https://mempool.space";
const MEMPOOL_FEE_RATE_URL_SIGNET: &str = "https://mempool.space/signet";
const MEMPOOL_FEE_RATE_URL_TESTNET: &str = "https://mempool.space/testnet";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FeeRate {
    pub fastest_fee: usize,
    pub half_hour_fee: usize,
    pub hour_fee: usize,
    pub economy_fee: usize,
    pub minimum_fee: usize,
}

impl FeeRate {
    fn local_fee_rate() -> Self {
        FeeRate {
            // we on purpose have different values to see an effect for clients asking for different
            // priorities
            fastest_fee: 5,
            half_hour_fee: 4,
            hour_fee: 3,
            economy_fee: 2,
            minimum_fee: 1,
        }
    }
}

#[derive(PartialEq)]
pub enum Network {
    Mainnet,
    Signet,
    Testnet,
    /// We assume a local regtest setup and will not perform any request to mempool.space
    Local,
}

pub struct MempoolFeeRateEstimator {
    url: String,
    network: Network,
}

impl MempoolFeeRateEstimator {
    pub fn new(network: Network) -> Self {
        let url = match network {
            Network::Mainnet => MEMPOOL_FEE_RATE_URL_MAINNET,
            Network::Signet => MEMPOOL_FEE_RATE_URL_SIGNET,
            Network::Testnet => MEMPOOL_FEE_RATE_URL_TESTNET,
            Network::Local => "http://thereisnosuchthingasabitcoinmempool.com",
        }
        .to_string();

        Self { url, network }
    }

    /// Fetches the latest fee rate from mempool.space
    ///
    /// Note: if self.network is Network::Local we will not perform a request to mempool.space but
    /// return static values
    pub fn fetch_fee_sync(&self) -> Result<FeeRate> {
        if Network::Local == self.network {
            return Ok(FeeRate::local_fee_rate());
        }
        let url = format!("{}/api/v1/fees/recommended", self.url);
        let response = reqwest::blocking::get(url)?;
        let fee_rate = response.json()?;
        Ok(fee_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // we keep this test running on  CI even though it connects to the internet. This allows us to
    // be notified if the API ever changes
    #[test]
    pub fn test_fetching_fee_rate_from_mempool_sync() {
        let mempool = MempoolFeeRateEstimator::new(Network::Testnet);
        let _testnet_fee_rate = mempool.fetch_fee_sync().unwrap();
        let mempool = MempoolFeeRateEstimator::new(Network::Mainnet);
        let _testnet_fee_rate = mempool.fetch_fee_sync().unwrap();
        let mempool = MempoolFeeRateEstimator::new(Network::Signet);
        let _testnet_fee_rate = mempool.fetch_fee_sync().unwrap();
        let mempool = MempoolFeeRateEstimator::new(Network::Local);
        let _testnet_fee_rate = mempool.fetch_fee_sync().unwrap();
    }
}
