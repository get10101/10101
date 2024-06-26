use crate::notifications::Notification;
use crate::notifications::NotificationKind;
use crate::routes::AppState;
use crate::AppError;
use axum::extract::State;
use axum::Json;
use bitcoin::secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushCampaignParams {
    pub node_ids: Vec<PublicKey>,
    pub title: String,
    pub message: String,
    pub dry_run: Option<bool>,
}

#[instrument(skip_all, err(Debug))]
pub async fn post_push_campaign(
    State(state): State<Arc<AppState>>,
    params: Json<PushCampaignParams>,
) -> Result<String, AppError> {
    let params = params.0;
    tracing::info!(?params, "Sending campaign with push notifications");

    let notification_kind = NotificationKind::Custom {
        title: params.title.clone(),
        message: params.message.clone(),
    };

    tracing::info!(
        params.title,
        params.message,
        receivers = params.node_ids.len(),
        "Sending push notification campaign",
    );

    if params.dry_run.unwrap_or(true) {
        tracing::debug!("Not sending push notification campaign because of dry run flag.");
    } else {
        state
            .notification_sender
            .send(Notification::new_batch(
                params.clone().node_ids,
                notification_kind,
            ))
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to send push notifications: {e:#}"))
            })?;
    }

    Ok(format!(
        "Sending push notification campaign (title: {}, message: {} to {} users",
        params.title,
        params.message,
        params.node_ids.len(),
    ))
}
