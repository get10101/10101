use tokio::runtime::Runtime;
use tokio::select;
use tokio::sync::watch;

use crate::event::EventInternal;
use crate::event::{self};

/// Services which status is monitored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    Orderbook,
}

/// Health status of the node
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    #[default]
    Unknown,
    Online,
    Offline,
}

pub type ServiceUpdate = (Service, ServiceStatus);

pub struct Tx {
    pub orderbook: watch::Sender<ServiceStatus>,
}

/// Entity that gathers all the health data and sends notifications
pub struct Health {
    _monitoring_task: tokio::task::JoinHandle<()>,
}

impl Health {
    pub fn new(runtime: &Runtime) -> (Self, Tx) {
        let (orderbook_tx, mut orderbook_rx) = watch::channel(ServiceStatus::Unknown);

        let _monitoring_task = runtime.spawn(async move {
            loop {
                select! {
                    result = orderbook_rx.changed() => {
                        match result {
                            Ok(()) => {
                                let status = orderbook_rx.borrow();
                                event::publish(&EventInternal::ServiceHealthUpdate((Service::Orderbook, *status)));
                            }
                            Err(_) => {
                                tracing::error!("Sender dropped");
                                event::publish(&EventInternal::ServiceHealthUpdate((Service::Orderbook, ServiceStatus::Unknown)));
                                break;
                            },
                        }
                    },
                }
            }
        });

        (
            Self { _monitoring_task },
            Tx {
                orderbook: orderbook_tx,
            },
        )
    }
}
