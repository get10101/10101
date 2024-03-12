use axum::async_trait;
use axum_login::tower_sessions::session::Id;
use axum_login::tower_sessions::session::Record;
use axum_login::tower_sessions::session_store;
use axum_login::tower_sessions::ExpiredDeletion;
use axum_login::tower_sessions::SessionStore;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct InMemorySessionStore {
    sessions: Arc<RwLock<HashMap<Id, Record>>>,
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn save(&self, session_record: &Record) -> session_store::Result<()> {
        self.sessions
            .write()
            .insert(session_record.id, session_record.clone());
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        Ok(self.sessions.read().get(session_id).cloned())
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        self.sessions.write().remove(session_id);
        Ok(())
    }
}

#[async_trait]
impl ExpiredDeletion for InMemorySessionStore {
    async fn delete_expired(&self) -> session_store::Result<()> {
        let mut expired_session_ids = vec![];
        let sessions = self.sessions.read();
        for session in sessions.iter() {
            if OffsetDateTime::now_utc() >= session.1.expiry_date {
                expired_session_ids.push(session.0);
            }
        }
        for expired_session_id in expired_session_ids.iter() {
            self.sessions.write().remove(expired_session_id);
        }
        Ok(())
    }
}

impl InMemorySessionStore {
    pub(crate) fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) async fn continuously_delete_expired(
        self,
        period: tokio::time::Duration,
    ) -> session_store::Result<()> {
        let mut interval = tokio::time::interval(period);
        loop {
            self.delete_expired().await?;
            interval.tick().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use time::ext::NumericalDuration;

    #[tokio::test]
    async fn delete_expired_method_deadlocks() {
        std::panic::set_hook(Box::new(|_| {
            // The timeout task panicked because we ran into a deadlock.
            std::process::exit(0);
        }));

        let expired_date = OffsetDateTime::now_utc() - 10.minutes();

        let sessions = Arc::new(RwLock::new(HashMap::from_iter([(
            Id::default(),
            Record {
                id: Id::default(),
                data: HashMap::default(),
                expiry_date: expired_date,
            },
        )])));
        let store = InMemorySessionStore { sessions };

        // Timeout task spawned to detect the deadlock.
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(5));
            panic!("Ran into deadlock");
        });

        // This method deadlocks.
        let _ = store.delete_expired().await;

        std::process::exit(1);
    }
}
