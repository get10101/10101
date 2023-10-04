use crate::db::positions_helper::get_non_expired_positions_joined_with_fcm_token;
use crate::notifications::send_rollover_reminder;
use crate::notifications::Notification;
use crate::settings::Settings;
use anyhow::Result;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use tokio::sync::mpsc::Sender;
use tokio_cron_scheduler::Job;
use tokio_cron_scheduler::JobScheduler;

pub struct NotificationScheduler {
    scheduler: JobScheduler,
    sender: Sender<Notification>,
    settings: Settings,
}

impl NotificationScheduler {
    pub async fn new(sender: Sender<Notification>, settings: Settings) -> Self {
        let scheduler = JobScheduler::new()
            .await
            .expect("To be able to start the scheduler");

        Self {
            scheduler,
            sender,
            settings,
        }
    }

    pub async fn add_rollover_window_reminder_job(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Result<()> {
        let sender = self.sender.clone();
        let schedule = self.settings.rollover_window_open_scheduler.clone();

        let uuid = self
            .scheduler
            .add(
                Job::new_async(schedule.as_str(), move |_, _| {
                    let sender = sender.clone();
                    let mut conn = pool.get().expect("To be able to get a db connection");
                    match get_non_expired_positions_joined_with_fcm_token(&mut conn) {
                        Ok(positions_with_token) => Box::pin(async move {
                            send_rollover_reminder(positions_with_token.as_slice(), &sender).await;
                        }),
                        Err(error) => Box::pin(async move {
                            tracing::error!("Could not load positions with fcm token {error:#}")
                        }),
                    }
                })
                .expect("To be able to add the job"),
            )
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
