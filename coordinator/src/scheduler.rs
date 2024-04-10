use crate::db;
use crate::db::orders_helper::get_all_matched_market_orders_by_order_reason;
use crate::message::OrderbookMessage;
use crate::node::Node;
use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use crate::referrals;
use crate::settings::Settings;
use anyhow::Result;
use bitcoin::Network;
use commons::OrderReason;
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

    pub async fn update_bonus_status_for_users(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Result<()> {
        let schedule = self.settings.update_user_bonus_status_scheduler.clone();

        let uuid = self
            .scheduler
            .add(build_update_bonus_status_job(schedule.as_str(), pool)?)
            .await?;
        tracing::debug!(
            job_id = uuid.to_string(),
            "Started new job to update users bonus status"
        );
        Ok(())
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

    pub async fn add_reminder_to_close_liquidated_position_job(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
    ) -> Result<()> {
        let sender = self.sender.clone();
        let schedule = self.settings.close_liquidated_position_scheduler.clone();

        let uuid = self
            .scheduler
            .add(build_remind_to_close_liquidated_position_notification_job(
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
        let mut conn = match pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                return Box::pin(async move {
                    tracing::error!("Failed to get connection. Error: {e:#}")
                });
            }
        };

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
                        if let Err(e) = node
                            .check_rollover(
                                position.trader,
                                position.expiry_timestamp,
                                node.inner.network,
                                &notifier,
                                Some(notification.clone()),
                            )
                            .await
                        {
                            tracing::error!(trader_id=%position.trader, "Failed to check rollover. {e:#}");
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

fn build_update_bonus_status_job(
    schedule: &str,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<Job, JobSchedulerError> {
    Job::new_async(schedule, move |_, _| {
        let mut conn = match pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                return Box::pin(async move {
                    tracing::error!("Failed to get connection. Error: {e:#}")
                });
            }
        };

        match referrals::update_referral_status(&mut conn) {
            Ok(number_of_updated_users) => Box::pin({
                async move {
                    tracing::debug!(
                        number_of_updated_users,
                        "Successfully updated users bonus status."
                    )
                }
            }),
            Err(error) => {
                Box::pin(
                    async move { tracing::error!("Could not load update bonus status {error:#}") },
                )
            }
        }
    })
}

fn build_remind_to_close_expired_position_notification_job(
    schedule: &str,
    notification_sender: mpsc::Sender<Notification>,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<Job, JobSchedulerError> {
    Job::new_async(schedule, move |_, _| {
        let notification_sender = notification_sender.clone();
        let mut conn = match pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                return Box::pin(async move {
                    tracing::error!("Failed to get connection. Error: {e:#}")
                });
            }
        };

        // Note, positions that are expired longer than
        // [`crate::node::expired_positions::EXPIRED_POSITION_TIMEOUT`] are set to closing, hence
        // those positions will not get notified anymore afterwards.
        match get_all_matched_market_orders_by_order_reason(&mut conn, vec![OrderReason::Expired]) {
            Ok(positions_with_token) => Box::pin({
                async move {
                    for (order, fcm_token) in positions_with_token {
                        tracing::debug!(trader_id=%order.trader_id, "Sending reminder to close expired position.");
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

fn build_remind_to_close_liquidated_position_notification_job(
    schedule: &str,
    notification_sender: mpsc::Sender<Notification>,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<Job, JobSchedulerError> {
    Job::new_async(schedule, move |_, _| {
        let notification_sender = notification_sender.clone();
        let mut conn = match pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                return Box::pin(async move {
                    tracing::error!("Failed to get connection. Error: {e:#}")
                });
            }
        };

        // Note, positions that are liquidated longer than
        // [`crate::node::liquidated_positions::LIQUIDATED_POSITION_TIMEOUT`] are set to closing,
        // hence those positions will not get notified anymore afterwards.
        match get_all_matched_market_orders_by_order_reason(
            &mut conn,
            vec![
                OrderReason::TraderLiquidated,
                OrderReason::CoordinatorLiquidated,
            ],
        ) {
            Ok(orders_with_token) => Box::pin({
                async move {
                    for (order, fcm_token) in orders_with_token {
                        tracing::debug!(trader_id=%order.trader_id, "Sending reminder to close liquidated position.");

                        let notification_kind = NotificationKind::Custom {
                            title: "Pending liquidation 💸".to_string(),
                            message: "Open your app to execute the liquidation ".to_string(),
                        };

                        if let Err(e) = notification_sender
                            .send(Notification::new(
                                fcm_token.clone(),
                                notification_kind.clone(),
                            ))
                            .await
                        {
                            tracing::error!(
                                "Failed to send {:?} notification: {e:?}",
                                notification_kind
                            );
                        }
                    }
                }
            }),
            Err(error) => Box::pin(async move {
                tracing::error!("Could not load orders with fcm token {error:#}")
            }),
        }
    })
}
