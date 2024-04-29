use anyhow::Context;
use anyhow::Result;
use bitcoin::address::NetworkUnchecked;
use bitcoin::Address;
use native::api::ContractSymbol;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

/// A wrapper over the coordinator HTTP API.
///
/// It does not aim to be complete, functionality will be added as needed.
pub struct Coordinator {
    client: Client,
    host: String,
    db_host: String,
}

impl Coordinator {
    pub fn new(client: Client, host: &str, db_host: &str) -> Self {
        Self {
            client,
            host: host.to_string(),
            db_host: db_host.to_string(),
        }
    }

    pub fn new_local(client: Client) -> Self {
        Self::new(client, "http://localhost:8000", "http://localhost:3002")
    }

    /// Check whether the coordinator is running.
    pub async fn is_running(&self) -> bool {
        self.get(format!("{}/health", self.host)).await.is_ok()
    }

    pub async fn sync_node(&self) -> Result<()> {
        self.post::<()>(format!("{}/api/admin/sync", self.host), None)
            .await?;

        Ok(())
    }

    pub async fn get_balance(&self) -> Result<Balance> {
        let balance = self
            .get(format!("{}/api/admin/wallet/balance", self.host))
            .await?
            .json()
            .await?;

        Ok(balance)
    }

    pub async fn get_new_address(&self) -> Result<Address<NetworkUnchecked>> {
        Ok(self
            .get(format!("{}/api/newaddress", self.host))
            .await?
            .text()
            .await?
            .parse()?)
    }

    pub async fn get_dlc_channels(&self) -> Result<Vec<DlcChannelDetails>> {
        Ok(self
            .get(format!("{}/api/admin/dlc_channels", self.host))
            .await?
            .json()
            .await?)
    }

    pub async fn rollover(&self, dlc_channel_id: &str) -> Result<reqwest::Response> {
        self.post::<()>(
            format!("{}/api/admin/rollover/{dlc_channel_id}", self.host),
            None,
        )
        .await
    }

    pub async fn get_positions(&self, trader_pubkey: &str) -> Result<Vec<Position>> {
        let positions = self
            .get(format!(
                "{}/positions?trader_pubkey=eq.{trader_pubkey}",
                self.db_host
            ))
            .await?
            .json()
            .await?;

        Ok(positions)
    }

    pub async fn collaborative_revert(
        &self,
        request: CollaborativeRevertCoordinatorRequest,
    ) -> Result<()> {
        self.post(
            format!("{}/api/admin/channels/revert", self.host),
            Some(request),
        )
        .await?;

        Ok(())
    }

    pub async fn post_funding_rates(&self, request: FundingRates) -> Result<()> {
        self.post(
            format!("{}/api/admin/funding-rates", self.host),
            Some(request),
        )
        .await?;

        Ok(())
    }

    /// Modify the `creation_timestamp` of the trader positions stored in the coordinator database.
    ///
    /// This can be used together with `post_funding_rates` to force the coordinator to generate a
    /// funding fee event for a given position.
    pub async fn modify_position_creation_timestamp(
        &self,
        timestamp: OffsetDateTime,
        trader_pubkey: &str,
    ) -> Result<()> {
        #[derive(Serialize)]
        struct Request {
            #[serde(with = "time::serde::rfc3339")]
            creation_timestamp: OffsetDateTime,
        }

        self.patch(
            format!(
                "{}/positions?trader_pubkey=eq.{trader_pubkey}",
                self.db_host
            ),
            Some(Request {
                creation_timestamp: timestamp,
            }),
        )
        .await?;

        Ok(())
    }

    pub async fn get_funding_fee_events(
        &self,
        trader_pubkey: &str,
        position_id: u64,
    ) -> Result<Vec<FundingFeeEvent>> {
        let funding_fee_events = self
            .get(format!(
                "{}/funding_fee_events?trader_pubkey=eq.{trader_pubkey}&position_id=eq.{position_id}",
                self.db_host
            ))
            .await?
            .json()
            .await?;

        Ok(funding_fee_events)
    }

    async fn get(&self, path: String) -> Result<reqwest::Response> {
        self.client
            .get(path)
            .send()
            .await
            .context("Could not send GET request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }

    async fn post<T: Serialize>(&self, path: String, body: Option<T>) -> Result<reqwest::Response> {
        let request = self.client.post(path);

        let request = match body {
            Some(ref body) => {
                let body = serde_json::to_string(body)?;
                request
                    .header("Content-Type", "application/json")
                    .body(body)
            }
            None => request,
        };

        request
            .send()
            .await
            .context("Could not send POST request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }

    async fn patch<T: Serialize>(
        &self,
        path: String,
        body: Option<T>,
    ) -> Result<reqwest::Response> {
        let request = self.client.patch(path);

        let request = match body {
            Some(ref body) => {
                let body = serde_json::to_string(body)?;
                request
                    .header("Content-Type", "application/json")
                    .body(body)
            }
            None => request,
        };

        request
            .send()
            .await
            .context("Could not send PATCH request to coordinator")?
            .error_for_status()
            .context("Coordinator did not return 200 OK")
    }
}

#[derive(Deserialize, Debug)]
pub struct Balance {
    pub onchain: u64,
    pub dlc_channel: u64,
}

#[derive(Deserialize, Debug)]
pub struct DlcChannels {
    #[serde(flatten)]
    pub channels: Vec<DlcChannelDetails>,
}

#[derive(Deserialize, Debug)]
pub struct DlcChannelDetails {
    pub dlc_channel_id: Option<String>,
    pub counter_party: String,
    pub channel_state: ChannelState,
    pub signed_channel_state: Option<SignedChannelState>,
    pub update_idx: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub enum ChannelState {
    Offered,
    Accepted,
    Signed,
    Closing,
    Closed,
    CounterClosed,
    ClosedPunished,
    CollaborativelyClosed,
    FailedAccept,
    FailedSign,
    Cancelled,
}

#[derive(Deserialize, Debug, PartialEq)]
pub enum SignedChannelState {
    Established,
    SettledOffered,
    SettledReceived,
    SettledAccepted,
    SettledConfirmed,
    Settled,
    RenewOffered,
    RenewAccepted,
    RenewConfirmed,
    RenewFinalized,
    Closing,
    CollaborativeCloseOffered,
}

#[derive(Serialize)]
pub struct CollaborativeRevertCoordinatorRequest {
    pub channel_id: String,
    pub fee_rate_sats_vb: u64,
    pub counter_payout: u64,
    pub price: Decimal,
}

#[derive(Debug, Deserialize, Clone)]
// For `insta`.
#[derive(Serialize)]
pub struct Position {
    pub id: u64,
    pub contract_symbol: ContractSymbol,
    pub trader_leverage: Decimal,
    pub quantity: Decimal,
    pub trader_direction: Direction,
    pub average_entry_price: Decimal,
    pub trader_liquidation_price: Decimal,
    pub position_state: PositionState,
    pub coordinator_margin: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub creation_timestamp: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expiry_timestamp: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub update_timestamp: OffsetDateTime,
    pub trader_pubkey: String,
    pub temporary_contract_id: Option<String>,
    pub trader_realized_pnl_sat: Option<i64>,
    pub trader_unrealized_pnl_sat: Option<i64>,
    pub closing_price: Option<Decimal>,
    pub coordinator_leverage: Decimal,
    pub trader_margin: i64,
    pub coordinator_liquidation_price: Decimal,
    pub order_matching_fees: i64,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
// For `insta`.
#[derive(Serialize)]
pub enum Direction {
    Long,
    Short,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
// For `insta`.
#[derive(Serialize)]
pub enum PositionState {
    Proposed,
    Open,
    Closing,
    Rollover,
    Closed,
    Failed,
    Resizing,
}

#[derive(Debug, Serialize)]
pub struct FundingRates(pub Vec<FundingRate>);

#[derive(Debug, Serialize)]
pub struct FundingRate {
    pub rate: Decimal,
    #[serde(with = "time::serde::rfc3339")]
    pub start_date: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub end_date: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct FundingFeeEvent {
    pub amount_sats: i64,
    pub trader_pubkey: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub paid_date: Option<OffsetDateTime>,
    pub position_id: u64,
}
