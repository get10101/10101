use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use reqwest::Client;
use reqwest::StatusCode;
use reqwest::Url;
use serde::Serialize;
use std::time::Duration;
use tokio::sync::watch;

/// Health status of a service
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ServiceStatus {
    #[default]
    Unknown,
    Online,
    Offline,
}

/// Health monitoring for the node
///
/// Simple endpoint querying is handled by provided configuration, for more complex health checks
/// the transmitters are exposed to be plugged in the services that need to publish their health
/// status.

#[derive(Clone)]
pub struct Health {
    /// Coordinator HTTP API status
    coordinator_rx: watch::Receiver<ServiceStatus>,
    /// Orderbook websocket stream status
    orderbook_rx: watch::Receiver<ServiceStatus>,
    /// Bitmex pricefeed stream status
    bitmex_pricefeed_rx: watch::Receiver<ServiceStatus>,
}

/// Transmitters that need to be plugged in the services that need to publish their health status.
pub struct Tx {
    pub orderbook: watch::Sender<ServiceStatus>,
    pub coordinator: watch::Sender<ServiceStatus>,
    pub bitmex_pricefeed: watch::Sender<ServiceStatus>,
}

/// Struct returned by maker's health endpoint.
#[derive(Debug, Serialize)]
pub struct OverallMakerHealth {
    coordinator: ServiceStatus,
    orderbook: ServiceStatus,
    bitmex_pricefeed: ServiceStatus,
}

impl OverallMakerHealth {
    pub fn is_healthy(&self) -> bool {
        self.coordinator == ServiceStatus::Online
            && self.bitmex_pricefeed == ServiceStatus::Online
            && self.orderbook == ServiceStatus::Online
    }
}

impl Health {
    pub fn new() -> (Self, Tx) {
        let (orderbook_tx, orderbook_rx) = watch::channel(ServiceStatus::Unknown);
        let (coordinator_tx, coordinator_rx) = watch::channel(ServiceStatus::Unknown);
        let (bitmex_pricefeed_tx, bitmex_pricefeed_rx) = watch::channel(ServiceStatus::Unknown);

        (
            Self {
                coordinator_rx,
                orderbook_rx,
                bitmex_pricefeed_rx,
            },
            Tx {
                orderbook: orderbook_tx,
                coordinator: coordinator_tx,
                bitmex_pricefeed: bitmex_pricefeed_tx,
            },
        )
    }

    pub fn get_health(&self) -> Result<OverallMakerHealth> {
        let health_info = OverallMakerHealth {
            coordinator: self.get_coordinator_status(),
            orderbook: self.get_orderbook_status(),
            bitmex_pricefeed: self.get_bitmex_pricefeed_status(),
        };

        match health_info.is_healthy() {
            true => Ok(health_info),
            false => {
                bail!("Status: ERROR\n + {health_info:?}");
            }
        }
    }

    pub fn get_coordinator_status(&self) -> ServiceStatus {
        *self.coordinator_rx.borrow()
    }

    pub fn get_orderbook_status(&self) -> ServiceStatus {
        *self.orderbook_rx.borrow()
    }

    pub fn get_bitmex_pricefeed_status(&self) -> ServiceStatus {
        *self.bitmex_pricefeed_rx.borrow()
    }
}

/// Simple way of checking if a service is online or offline
pub async fn check_health_endpoint(
    client: &Client,
    endpoint: Url,
    tx: watch::Sender<ServiceStatus>,
    interval: Duration,
) {
    loop {
        let status = if check_endpoint_availability(client, endpoint.clone())
            .await
            .is_ok()
        {
            ServiceStatus::Online
        } else {
            ServiceStatus::Offline
        };

        tx.send(status).expect("Receiver not to be dropped");
        tokio::time::sleep(interval).await;
    }
}

async fn check_endpoint_availability(client: &Client, endpoint: Url) -> Result<StatusCode> {
    tracing::trace!(%endpoint, "Sending request to check health");
    let response = client
        .get(endpoint)
        .send()
        .await
        .context("could not send request")?
        .error_for_status()?;
    Ok(response.status())
}
