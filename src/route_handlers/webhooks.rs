use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db_service;

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    message: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhookRequest {
    beacon_url: String,
}

pub async fn handler(Json(payload): Json<WebhookRequest>) -> impl axum::response::IntoResponse {
    tracing::debug!("Webhook request: {:?}", payload);
    db_service::set_beacon_url(Some(payload.beacon_url)).await;
    axum::http::status::StatusCode::OK
}
