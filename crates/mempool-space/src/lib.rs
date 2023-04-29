use anyhow::Result;
use serde::Deserialize;

#[derive(Clone)]
pub enum Network {
    Regtest,
    Mainnet,
    Testnet,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeeRecommendation {
    /// next block
    fastest_fee: u32,
    /// 3 blocks
    half_hour_fee: u32,
    /// 6 blocks
    hour_fee: u32,
    /// min relay fee
    minimum_fee: u32,
}

impl FeeRecommendation {
    pub fn fastest(&self) -> u32 {
        self.fastest_fee
    }
}

impl Default for FeeRecommendation {
    fn default() -> Self {
        Self {
            fastest_fee: 1,
            half_hour_fee: 1,
            hour_fee: 1,
            minimum_fee: 1,
        }
    }
}

async fn fetch_fee_recommendation(network: &Network) -> Result<FeeRecommendation> {
    let url = match network {
        Network::Mainnet => "https://mempool.space/api/v1/fees/recommended",
        Network::Testnet => "https://mempool.space/testnet/api/v1/fees/recommended",
        Network::Regtest => {
            return Ok(FeeRecommendation::default());
        }
    };

    let response = reqwest::get(url).await?;

    let fee_recommendation = response.json().await?;
    Ok(fee_recommendation)
}

fn fetch_fee_recommendation_blocking(network: &Network) -> Result<FeeRecommendation> {
    let url = match network {
        Network::Mainnet => "https://mempool.space/api/v1/fees/recommended",
        Network::Testnet => "https://mempool.space/testnet/api/v1/fees/recommended",
        Network::Regtest => {
            return Ok(FeeRecommendation::default());
        }
    };
    let response = reqwest::blocking::get(url)?;

    let fee_recommendation = response.json()?;
    Ok(fee_recommendation)
}

pub struct MempoolFeeEstimator {
    network: Network,
}

impl MempoolFeeEstimator {
    pub fn new(network: Network) -> MempoolFeeEstimator {
        Self { network }
    }

    /// Returns fee rate recommendation from mempool.space for specific block target
    ///
    /// For regtest it returns a default fee rate of 1sat/vbyte for all targets
    pub async fn estimate_fee(&self, target: u32) -> Result<u32> {
        let fee_rate = fetch_fee_recommendation(&self.network).await?;
        match target {
            0..=1 => Ok(fee_rate.fastest_fee),
            2..=3 => Ok(fee_rate.half_hour_fee),
            4..=6 => Ok(fee_rate.hour_fee),
            _ => Ok(fee_rate.minimum_fee),
        }
    }

    /// Returns fee rate recommendation from mempool.space for specific block target
    ///
    /// For regtest it returns a default fee rate of 1sat/vbyte for all targets
    pub fn estimate_fee_blocking(&self, target: u32) -> Result<u32> {
        let fee_rate = fetch_fee_recommendation_blocking(&self.network)?;
        match target {
            0..=1 => Ok(fee_rate.fastest_fee),
            2..=3 => Ok(fee_rate.half_hour_fee),
            4..=6 => Ok(fee_rate.hour_fee),
            _ => Ok(fee_rate.minimum_fee),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::FeeRecommendation;
    use crate::MempoolFeeEstimator;
    use crate::Network;

    #[test]
    fn serialization_test() {
        let json = r#"{
          "fastestFee": 1,
          "halfHourFee": 2,
          "hourFee": 3,
          "minimumFee": 4
        }"#;

        let fee_recommendation = serde_json::from_str::<FeeRecommendation>(json).unwrap();
        assert_eq!(
            fee_recommendation,
            FeeRecommendation {
                fastest_fee: 1,
                half_hour_fee: 2,
                hour_fee: 3,
                minimum_fee: 4
            }
        )
    }

    #[tokio::test]
    async fn integration_test_with_target() {
        let fee_estimator = MempoolFeeEstimator::new(Network::Mainnet);
        let fee_recommendation = fee_estimator.estimate_fee(1).await.unwrap();
        assert!(fee_recommendation >= 1);
    }

    #[test]
    fn integration_test_with_target_blocking() {
        let fee_estimator = MempoolFeeEstimator::new(Network::Mainnet);
        let fee_recommendation = fee_estimator.estimate_fee_blocking(1).unwrap();
        assert!(fee_recommendation >= 1);
    }
}
