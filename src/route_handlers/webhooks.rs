use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, time::Instant};
use tracing::log::{debug, error, info};
use webhook::client::WebhookClient;

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

    let mut state = state.lock().await;

    let current_status = state.is_troy_on_the_trails;
    let new_status = payload.on_the_trail;
    if current_status != new_status {
        info!(
            "Troy status changed from {} to {}",
            current_status, new_status
        );

        let content = format!(
            "Troy is {} on the trails!",
            if new_status { "now" } else { "no longer" }
        );

        tokio::spawn(async move {
            send_discord_webhook(&content).await;
        });
    }

    state.is_troy_on_the_trails = payload.on_the_trail;
    state.troy_status_last_updated = Some(Instant::now());

    axum::http::status::StatusCode::OK
}

async fn send_discord_webhook(content: &str) {
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            debug!("No Discord webhook URL found, skipping");
            return;
        }
    };

    let host_uri = crate::env_utils::get_host_uri(None);
    let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

    let client: WebhookClient = WebhookClient::new(&webhook_url);

    match client
        .send(|message| {
            message
                .username("TOTT")
                .avatar_url(avatar_url)
                .embed(|embed| {
                    embed.title(content).footer(
                        "Powered by troyonthetrails.com",
                        Some(avatar_url.to_string()),
                    )
                })
        })
        .await
    {
        Ok(_) => debug!("Successfully sent Discord webhook"),
        Err(e) => {
            error!("Failed to send Discord webhook: {}", e);
        }
    }
}
