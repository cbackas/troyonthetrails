use webhook::{
    client::WebhookClient,
    models::{Embed, Message},
};

use crate::{
    strava::{self, api_service::Activity},
    utils,
};

struct TOTTWebhook {
    troy_status: bool,
    webhook_data: Option<WebhookData>,
}

struct WebhookData {
    name: Option<String>,
    distance: f64,
    total_elevation_gain: f64,
    average_speed: f64,
    max_speed: f64,
}

impl Into<webhook::models::Message> for TOTTWebhook {
    fn into(self) -> webhook::models::Message {
        let host_uri = crate::env_utils::get_host_uri();
        let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

        let embed = {
            let mut embed: Embed = Embed::new();
            embed
                .title(match self.troy_status {
                    true => "Troy is on the trails!",
                    false => "Troy is no longer on the trails!",
                })
                .footer(
                    "Powered by troyonthetrails.com",
                    Some(avatar_url.to_string()),
                );

            if let Some(webhook_data) = &self.webhook_data {
                if let Some(name) = &webhook_data.name {
                    embed.description(name);
                }

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
        };

        Message {
            content: None,
            username: Some("TOTT".to_string()),
            avatar_url: Some(avatar_url.to_string()),
            tts: false,
            embeds: vec![embed],
            allow_mentions: None,
            action_rows: vec![],
        }
    }
}

async fn send_webhook(webhook_data: TOTTWebhook) {
    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            tracing::debug!("No Discord webhook URL found, skipping");
            return;
        }
    };

    let client: WebhookClient = WebhookClient::new(&webhook_url);
    match client.send_message(&webhook_data.into()).await {
        Ok(_) => {
            tracing::debug!("Successfully sent Discord webhook");
        }
        Err(e) => {
            tracing::error!("Failed to send Discord webhook: {}", e);
        }
    }
}

pub async fn send_starting_webhook() {
    send_webhook(TOTTWebhook {
        troy_status: true,
        webhook_data: None,
    })
    .await;
}

pub async fn send_end_webhook(activity_id: Option<i64>) {
    let strava_stats: Option<WebhookData> = {
        let last_activity: Option<Activity> = match strava::api_service::get_recent_activity().await
        {
            Ok(activity) => Some(activity),
            Err(e) => {
                tracing::error!("Failed to get last activity: {}", e);
                None
            }
        };

        match last_activity {
            None => {
                tracing::error!("No last activity found");
                None
            }

            Some(last_activity) => {
                let name = match last_activity.name.clone().as_str() {
                    "Afternoon Mountain Bike Ride" => None,
                    "Morning Mountain Bike Ride" => None,
                    "Evening Mountain Bike Ride" => None,
                    "Lunch Mountain Bike Ride" => None,
                    _ => Some(last_activity.name),
                };
                let distance = utils::meters_to_miles(last_activity.distance, false);
                let total_elevation_gain =
                    utils::meters_to_feet(last_activity.total_elevation_gain, true);
                let average_speed = utils::mps_to_miph(last_activity.average_speed, false);
                let max_speed = utils::mps_to_miph(last_activity.max_speed, false);
                Some(WebhookData {
                    name,
                    distance,
                    total_elevation_gain,
                    average_speed,
                    max_speed,
                })
            }
        }
    };

    send_webhook(TOTTWebhook {
        troy_status: false,
        webhook_data: strava_stats,
    })
    .await;
}
