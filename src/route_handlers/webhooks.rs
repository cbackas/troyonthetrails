use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::log::{debug, error, info};
use webhook::{client::WebhookClient, models::Message};

use crate::{
    db_service::{get_troy_status, set_troy_status},
    strava_api_service::Activity,
    utils::{meters_to_feet, meters_to_miles, mps_to_miph},
    API_SERVICE,
};

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    message: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhookRequest {
    on_the_trail: bool,
}

struct WebhookData {
    distance: f64,
    total_elevation_gain: f64,
    average_speed: f64,
    max_speed: f64,
}

pub async fn handler(Json(payload): Json<WebhookRequest>) -> impl axum::response::IntoResponse {
    debug!("Webhook request: {:?}", payload);

    let troy_status = get_troy_status().await;
    let current_status = troy_status.is_on_trail;
    let new_status = payload.on_the_trail;
    if current_status != new_status {
        info!(
            "Troy status changed from {} to {}",
            current_status, new_status
        );

        set_troy_status(new_status).await;

        tokio::spawn(async move {
            send_discord_webhook(new_status).await;
        });
    }

    axum::http::status::StatusCode::OK
}

async fn send_discord_webhook(is_on_the_trails: bool) {
    let strava_stats: Option<WebhookData> = match is_on_the_trails {
        true => None,
        false => {
            let mut api_service = API_SERVICE.lock().await;
            let last_activity: Option<Activity> = match api_service.get_recent_activity().await {
                Ok(activity) => Some(activity),
                Err(e) => {
                    error!("Failed to get last activity: {}", e);
                    None
                }
            };

            match last_activity {
                None => {
                    error!("No last activity found");
                    None
                }

                Some(last_activity) => {
                    let distance = meters_to_miles(last_activity.distance, false);
                    let total_elevation_gain =
                        meters_to_feet(last_activity.total_elevation_gain, true);
                    let average_speed = mps_to_miph(last_activity.average_speed, false);
                    let max_speed = mps_to_miph(last_activity.max_speed, false);
                    Some(WebhookData {
                        distance,
                        total_elevation_gain,
                        average_speed,
                        max_speed,
                    })
                }
            }
        }
    };

    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            debug!("No Discord webhook URL found, skipping");
            return;
        }
    };

    let host_uri = crate::env_utils::get_host_uri();
    let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

    let message = &mut Message::new();
    message.username("TOTT").avatar_url(avatar_url);
    message.embed(|embed| {
        embed
            .title(match is_on_the_trails {
                true => "Troy is on the trails!",
                false => "Troy is no longer on the trails!",
            })
            .footer(
                "Powered by troyonthetrails.com",
                Some(avatar_url.to_string()),
            );

        if let Some(webhook_data) = &strava_stats {
            embed
                .field("Distance", &format!("{}mi", &webhook_data.distance), true)
                .field(
                    "Elevation Gain",
                    &format!("{}ft", &webhook_data.total_elevation_gain),
                    true,
                )
                .field(
                    "Average Speed",
                    &format!("{}mph", &webhook_data.average_speed),
                    true,
                )
                .field(
                    "Top Speed",
                    &format!("{}mph", &webhook_data.max_speed),
                    true,
                );
        };

        embed
    });

    let client: WebhookClient = WebhookClient::new(&webhook_url);

    match client.send_message(message).await {
        Ok(_) => {
            debug!("Successfully sent Discord webhook");
        }
        Err(e) => {
            error!("Failed to send Discord webhook: {}", e);
        }
    }
}
