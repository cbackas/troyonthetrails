use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, time::Instant};
use tracing::log::{debug, info};

use crate::AppState;

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    message: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhookRequest {
    on_the_trail: bool,
}

pub async fn handler(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(payload): Json<WebhookRequest>,
) -> impl axum::response::IntoResponse {
    debug!("Webhook request: {:?}", payload);
    info!("Setting troy status to: {}", payload.on_the_trail);

    let mut state = state.lock().await;
    state.is_troy_on_the_trails = payload.on_the_trail;
    state.last_updated = Some(Instant::now());

    axum::http::status::StatusCode::OK
}
