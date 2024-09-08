use shared_lib::structs::URLParams;
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

impl From<TOTTWebhook> for Embed {
    fn from(val: TOTTWebhook) -> Self {
        let mut embed: Embed = get_embed();
        embed.title(match val.troy_status {
            true => "Troy is on the trails!",
            false => "Troy is no longer on the trails!",
        });

        if let Some(webhook_data) = &val.webhook_data {
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
    }
}

impl From<TOTTWebhook> for Message {
    fn from(val: TOTTWebhook) -> Self {
        let host_uri = crate::env_utils::get_host_uri();
        let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

        Message {
            content: None,
            username: Some("TOTT".to_string()),
            avatar_url: Some(avatar_url.to_string()),
            tts: false,
            embeds: vec![val.into()],
            allow_mentions: None,
            action_rows: vec![],
        }
    }
}

struct StringMessage(String);

impl From<StringMessage> for Message {
    fn from(val: StringMessage) -> Self {
        let host_uri = crate::env_utils::get_host_uri();
        let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

        let mut embed: Embed = get_embed();
        embed.title(&val.0);

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

fn get_embed() -> Embed {
    let host_uri = crate::env_utils::get_host_uri();
    let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

    let mut embed: Embed = Embed::new();
    embed.footer(
        "Powered by troyonthetrails.com",
        Some(avatar_url.to_string()),
    );
    embed
}

async fn send_webhook(webhook_data: impl Into<Message>) {
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
        let activity: Option<Activity> = match activity_id {
            Some(activity_id) => match strava::api_service::get_activity(activity_id).await {
                Ok(activity) => Some(activity),
                Err(e) => {
                    tracing::error!("Failed to get last activity: {}", e);
                    None
                }
            },
            None => None,
        };

        match activity {
            None => {
                tracing::error!("No last activity found");
                None
            }

            Some(activity) => {
                let name = match activity.name.clone().as_str() {
                    "Afternoon Mountain Bike Ride" => None,
                    "Morning Mountain Bike Ride" => None,
                    "Evening Mountain Bike Ride" => None,
                    "Lunch Mountain Bike Ride" => None,
                    _ => Some(activity.name),
                };
                let distance = utils::meters_to_miles(activity.distance, false);
                let total_elevation_gain =
                    utils::meters_to_feet(activity.total_elevation_gain, true);
                let average_speed = utils::mps_to_miph(activity.average_speed, false);
                let max_speed = utils::mps_to_miph(activity.max_speed, false);
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

pub async fn send_discard_webhook() {
    send_webhook(StringMessage(
        "Troy has discarded the Strava activity".to_string(),
    ))
    .await;
}
