use crate::config::api::Config;
use crate::config::ConfigInternal;
use crate::event;
use crate::event::EventInternal;
use anyhow::Context;
use anyhow::Result;
use futures::future::RemoteHandle;
use futures::FutureExt;
use reqwest::StatusCode;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::watch;

/// Services which status is monitored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    Orderbook,
    Coordinator,
}

/// Health status of the node
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    #[default]
    Unknown,
    Online,
    Offline,
}

#[derive(Debug, Clone)]
pub struct ServiceUpdate {
    pub service: Service,
    pub status: ServiceStatus,
}

impl From<(Service, ServiceStatus)> for ServiceUpdate {
    fn from(tuple: (Service, ServiceStatus)) -> Self {
        let (service, status) = tuple;
        ServiceUpdate { service, status }
    }
}

/// Senders for the health status updates.
///
/// Meant to be injected into the services that need to publish their health status.
pub struct Tx {
    pub orderbook: watch::Sender<ServiceStatus>,
}

/// Entity that gathers all the service health data and publishes notifications
pub struct Health {
    _tasks: Vec<RemoteHandle<std::result::Result<(), tokio::task::JoinError>>>,
}

impl Health {
    pub fn new(config: Config, runtime: &Runtime) -> (Self, Tx) {
        let (orderbook_tx, orderbook_rx) = watch::channel(ServiceStatus::Unknown);

        let config: ConfigInternal = config.into();

        let mut tasks = Vec::new();

        let orderbook_monitoring = runtime
            .spawn(publish_status_updates(Service::Orderbook, orderbook_rx))
            .remote_handle()
            .1;
        tasks.push(orderbook_monitoring);

        let (coordinator_tx, coordinator_rx) = watch::channel(ServiceStatus::Unknown);

        let check_coordinator = runtime
            .spawn(check_health_endpoint(
                config.coordinator_health_endpoint(),
                coordinator_tx,
                config.health_check_interval(),
            ))
            .remote_handle()
            .1;
        tasks.push(check_coordinator);
        let coordinator_monitoring = runtime
            .spawn(publish_status_updates(Service::Coordinator, coordinator_rx))
            .remote_handle()
            .1;
        tasks.push(coordinator_monitoring);

        (
            Self { _tasks: tasks },
            Tx {
                orderbook: orderbook_tx,
            },
        )
    }
}

/// Publishes the health status updates for a given service to the event hub
async fn publish_status_updates(service: Service, mut rx: watch::Receiver<ServiceStatus>) {
    loop {
        match rx.changed().await {
            Ok(()) => {
                let status = rx.borrow();

                event::publish(&EventInternal::ServiceHealthUpdate(
                    (service, *status).into(),
                ));
            }
            Err(_) => {
                tracing::error!("Sender dropped");
                event::publish(&EventInternal::ServiceHealthUpdate(
                    (service, ServiceStatus::Unknown).into(),
                ));
                break;
            }
        }
    }
}

/// Periodically checks the health of a given service and updates the watch channel
async fn check_health_endpoint(
    endpoint: String,
    tx: watch::Sender<ServiceStatus>,
    interval: Duration,
) {
    loop {
        let status = if send_request(&endpoint).await.is_ok() {
            ServiceStatus::Online
        } else {
            ServiceStatus::Offline
        };

        tx.send(status).expect("Receiver not to be dropped");
        tokio::time::sleep(interval).await;
    }
}

// Returns the status code of the health endpoint, returning an error if the request fails
async fn send_request(endpoint: &str) -> Result<StatusCode> {
    tracing::trace!(%endpoint, "Sending request");
    let response = reqwest::get(endpoint)
        .await
        .context("could not send request")?
        .error_for_status()?;
    Ok(response.status())
}
