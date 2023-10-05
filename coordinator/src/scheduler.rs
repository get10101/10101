use crate::db::positions_helper::get_all_open_positions_with_expiry_before;
use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use crate::settings::Settings;
use anyhow::Result;
use bitcoin::Network;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use time::OffsetDateTime;
use tokio::sync::mpsc::Sender;
use tokio_cron_scheduler::Job;
use tokio_cron_scheduler::JobScheduler;
use tokio_cron_scheduler::JobSchedulerError;

pub struct NotificationScheduler {
    scheduler: JobScheduler,
    sender: Sender<Notification>,
    settings: Settings,
    network: Network,
}

impl NotificationScheduler {
    pub async fn new(sender: Sender<Notification>, settings: Settings, network: Network) -> Self {
        let scheduler = JobScheduler::new()
            .await
            .expect("To be able to start the scheduler");

        Self {
            scheduler,
            sender,
            settings,
            network,
        }
    }

    pub async fn add_rollover_window_reminder_job(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Result<()> {
        let sender = self.sender.clone();
        let schedule = self.settings.rollover_window_open_scheduler.clone();
        let network = self.network;

        let uuid = self
            .scheduler
            .add(build_notification_job(
                schedule.as_str(),
                sender,
                pool,
                network,
                NotificationKind::RolloverWindowOpen,
            )?)
            .await?;
        tracing::debug!(
            job_id = uuid.to_string(),
            "Started new job to remind rollover window is open"
        );
        Ok(())
    }

    pub async fn add_rollover_window_close_reminder_job(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Result<()> {
        let sender = self.sender.clone();
        let schedule = self.settings.rollover_window_close_scheduler.clone();
        let network = self.network;

        let uuid = self
            .scheduler
            .add(build_notification_job(
                schedule.as_str(),
                sender,
                pool,
                network,
                NotificationKind::PositionSoonToExpire,
            )?)
            .await?;

        tracing::debug!(
            job_id = uuid.to_string(),
            "Started new job to remind rollover window is open"
        );
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        self.scheduler.start().await?;
        Ok(())
    }
}

fn build_notification_job(
    schedule: &str,
    notification_sender: Sender<Notification>,
    pool: Pool<ConnectionManager<PgConnection>>,
    network: Network,
    notification: NotificationKind,
) -> Result<Job, JobSchedulerError> {
    Job::new_async(schedule, move |_, _| {
        let notification_sender = notification_sender.clone();
        let mut conn = pool.get().expect("To be able to get a db connection");

        if !coordinator_commons::is_eligible_for_rollover(OffsetDateTime::now_utc(), network) {
            return Box::pin(async move {
                tracing::warn!(
                                "Rollover window hasn't started yet. Job schedule seems to be miss-aligned with the rollover window. Skipping user notifications."
                            );
            });
        }

        // calculates the expiry of the next rollover window. positions which have an
        // expiry before that haven't rolled over yet, and need to be reminded.
        let expiry = coordinator_commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);
        match get_all_open_positions_with_expiry_before(&mut conn, expiry) {
            Ok(positions_with_token) => Box::pin({
                let notification = notification.clone();
                async move {
                    for (position, fcm_token) in positions_with_token {
                        tracing::debug!(trader_id=%position.trader, "Sending reminder to rollover position.");
                        if let Err(e) = notification_sender
                            .send(Notification::new(fcm_token.clone(), notification.clone()))
                            .await
                        {
                            tracing::error!("Failed to send {notification:?} notification: {e:?}");
                        }
                    }
                }
            }),
            Err(error) => Box::pin(async move {
                tracing::error!("Could not load positions with fcm token {error:#}")
            }),
        }
    })
}
