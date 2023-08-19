use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::log::info;

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
    info!("Webhook request: {:?}", payload);

    let mut state = state.lock().await;
    state.is_troy_on_the_trails = payload.on_the_trail;

    axum::http::status::StatusCode::OK
}
