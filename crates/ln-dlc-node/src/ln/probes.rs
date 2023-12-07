use anyhow::Result;
use lightning::ln::channelmanager::PaymentId;
use lightning::routing::router::Path;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::error::Elapsed;

#[derive(Clone, Debug)]
pub enum ProbeStatus {
    Succeeded { path: Path },
    Failed,
}

#[derive(Default)]
pub struct Probes {
    inner: RwLock<HashMap<PaymentId, Option<ProbeStatus>>>,
}

impl Probes {
    pub async fn register(&self, payment_id: PaymentId) {
        self.inner.write().await.insert(payment_id, None);
    }

    pub async fn update_status(&self, payment_id: PaymentId, new_status: ProbeStatus) {
        match self.inner.write().await.entry(payment_id) {
            Entry::Occupied(mut entry) => {
                let old_status = entry.insert(Some(new_status.clone()));

                tracing::debug!(%payment_id, ?old_status, ?new_status, "Updated probe");
            }
            Entry::Vacant(entry) => {
                entry.insert(Some(new_status.clone()));

                tracing::debug!(%payment_id, ?new_status, "Updating unregistered probe");
            }
        }
    }

    pub async fn wait_until_finished(
        &self,
        payment_id: &PaymentId,
        timeout: Duration,
    ) -> Result<ProbeStatus, Elapsed> {
        let status = tokio::time::timeout(timeout, async {
            loop {
                {
                    let guard = self.inner.read().await;

                    if let Some(Some(status)) = guard.get(payment_id) {
                        break status.clone();
                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(200)).await
            }
        })
        .await?;

        Ok(status)
    }
}

impl ProbeStatus {
    pub fn succeeded(path: Path) -> Self {
        Self::Succeeded { path }
    }
}
