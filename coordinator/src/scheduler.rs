use crate::db;
use crate::db::positions_helper::get_all_open_positions_with_expiry_before;
use crate::message::OrderbookMessage;
use crate::node::Node;
use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use crate::position::models::Position;
use crate::settings::Settings;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::Network;
use commons::Message;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use tokio_cron_scheduler::Job;
use tokio_cron_scheduler::JobScheduler;
use tokio_cron_scheduler::JobSchedulerError;

pub struct NotificationScheduler {
    scheduler: JobScheduler,
    sender: mpsc::Sender<Notification>,
    settings: Settings,
    network: Network,
    node: Node,
    notifier: mpsc::Sender<OrderbookMessage>,
}

impl NotificationScheduler {
    pub async fn new(
        sender: mpsc::Sender<Notification>,
        settings: Settings,
        network: Network,
        node: Node,
        notifier: mpsc::Sender<OrderbookMessage>,
    ) -> Self {
        let scheduler = JobScheduler::new()
            .await
            .expect("To be able to start the scheduler");

        Self {
            scheduler,
            sender,
            settings,
            network,
            node,
            notifier,
        }
    }

    pub async fn add_reminder_to_close_expired_position_job(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Result<()> {
        let sender = self.sender.clone();
        let schedule = self.settings.close_expired_position_scheduler.clone();

        let uuid = self
            .scheduler
            .add(build_remind_to_close_expired_position_notification_job(
                schedule.as_str(),
                sender,
                pool,
            )?)
            .await?;
        tracing::debug!(
            job_id = uuid.to_string(),
            "Started new job to remind to close an expired position"
        );
        Ok(())
    }

    pub async fn add_rollover_window_reminder_job(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Result<()> {
        let schedule = self.settings.rollover_window_open_scheduler.clone();
        let network = self.network;
        let node = self.node.clone();
        let notifier = self.notifier.clone();

        let uuid = self
            .scheduler
            .add(build_rollover_notification_job(
                schedule.as_str(),
                pool,
                network,
                NotificationKind::RolloverWindowOpen,
                node,
                notifier,
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
        let schedule = self.settings.rollover_window_close_scheduler.clone();
        let network = self.network;
        let node = self.node.clone();
        let notifier = self.notifier.clone();

        let uuid = self
            .scheduler
            .add(build_rollover_notification_job(
                schedule.as_str(),
                pool,
                network,
                NotificationKind::PositionSoonToExpire,
                node,
                notifier,
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

fn build_rollover_notification_job(
    schedule: &str,
    pool: Pool<ConnectionManager<PgConnection>>,
    network: Network,
    notification: NotificationKind,
    node: Node,
    notifier: mpsc::Sender<OrderbookMessage>,
) -> Result<Job, JobSchedulerError> {
    Job::new_async(schedule, move |_, _| {
        let notifier = notifier.clone();
        let mut conn = pool.get().expect("To be able to get a db connection");

        if !commons::is_eligible_for_rollover(OffsetDateTime::now_utc(), network) {
            return Box::pin(async move {
                tracing::warn!("Rollover window hasn't started yet. Job schedule seems to be miss-aligned with the rollover window. Skipping user notifications.");
            });
        }

        // calculates the expiry of the next rollover window. positions which have an
        // expiry before that haven't rolled over yet, and need to be reminded.
        let expiry = commons::calculate_next_expiry(OffsetDateTime::now_utc(), network);
        match db::positions::Position::get_all_open_positions_with_expiry_before(&mut conn, expiry)
        {
            Ok(positions) => Box::pin({
                tracing::debug!(
                    nr_of_positions = positions.len(),
                    "Found positions to rollover"
                );
                let notification = notification.clone();
                let node = node.clone();
                async move {
                    for position in positions {
                        if let Err(e) =
                            send_rollover_reminder(&notifier, &node, &position, &notification).await
                        {
                            tracing::error!(trader_id=%position.trader, "Failed to notify trader to rollover. {e:#}");
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

async fn send_rollover_reminder(
    notifier: &mpsc::Sender<OrderbookMessage>,
    node: &Node,
    position: &Position,
    notification: &NotificationKind,
) -> Result<()> {
    let trader_id = position.trader;
    tracing::debug!(%trader_id, "Sending rollover reminder.");

    let signed_channel = node.inner.get_signed_channel_by_trader_id(trader_id)?;

    tracing::debug!(%trader_id, position_id=position.id, "Proposing to rollover user's position");

    let message = OrderbookMessage::TraderMessage {
        trader_id,
        message: Message::Rollover(signed_channel.get_contract_id().map(hex::encode)),
        notification: Some(notification.clone()),
    };

    notifier.send(message).await.map_err(|e| anyhow!("{e:#}"))
}

fn build_remind_to_close_expired_position_notification_job(
    schedule: &str,
    notification_sender: mpsc::Sender<Notification>,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<Job, JobSchedulerError> {
    Job::new_async(schedule, move |_, _| {
        let notification_sender = notification_sender.clone();
        let mut conn = pool.get().expect("To be able to get a db connection");

        // Note, positions that are expired longer than
        // [`crate::node::expired_positions::EXPIRED_POSITION_TIMEOUT`] are set to closing, hence
        // those positions will not get notified anymore afterwards.
        match get_all_open_positions_with_expiry_before(&mut conn, OffsetDateTime::now_utc()) {
            Ok(positions_with_token) => Box::pin({
                async move {
                    for (position, fcm_token) in positions_with_token {
                        tracing::debug!(trader_id=%position.trader, "Sending reminder to close expired position.");
                        if let Err(e) = notification_sender
                            .send(Notification::new(
                                fcm_token.clone(),
                                NotificationKind::PositionExpired,
                            ))
                            .await
                        {
                            tracing::error!(
                                "Failed to send {:?} notification: {e:?}",
                                NotificationKind::PositionExpired
                            );
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
