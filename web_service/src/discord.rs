use serde::ser::SerializeStruct;

use shared_lib::structs::URLParams;

use crate::{
    strava::{self, api_service::Activity},
    utils,
};

struct OnTrailsNotification {
    beacon_url: String,
}

impl From<OnTrailsNotification> for DiscordEmbed {
    fn from(val: OnTrailsNotification) -> Self {
        let mut embed: DiscordEmbed = DiscordEmbed::default();
        embed.title("Troy is on the trails!");
        embed.description(&val.beacon_url);
        embed
    }
}

impl From<OnTrailsNotification> for DiscordMessage {
    fn from(val: OnTrailsNotification) -> Self {
        DiscordMessage {
            embed: Some(val.into()),
            ..Default::default()
        }
    }
}

struct OffTrailsNotification {
    webhook_data: Option<WebhookData>,
}

struct WebhookData {
    name: Option<String>,
    distance: f64,
    total_elevation_gain: f64,
    average_speed: f64,
    max_speed: f64,
    image: Option<WebhookImage>,
}
struct WebhookImage(Vec<u8>);

impl From<OffTrailsNotification> for DiscordEmbed {
    fn from(val: OffTrailsNotification) -> Self {
        let mut embed: DiscordEmbed = DiscordEmbed::default();

        embed.title("Troy is no longer on the trails!");

        let webhook_data = &val.webhook_data;
        if webhook_data.is_none() {
            return embed;
        }
        let webhook_data = webhook_data.as_ref().unwrap();

        if let Some(image) = &webhook_data.image {
            embed.image(EmbedImage::Bytes(ByteImageSource {
                bytes: image.0.clone(),
                file_name: "map_background.png".to_string(),
            }));
            tracing::debug!("Image found");
            return embed;
        } else {
            tracing::debug!("No image found");
        }

        if let Some(name) = &webhook_data.name {
            embed.description = Some(name.to_string());
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

        embed
    }
}

impl From<OffTrailsNotification> for DiscordMessage {
    fn from(val: OffTrailsNotification) -> Self {
        DiscordMessage {
            embed: Some(val.into()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct DiscordMessage {
    pub content: Option<String>,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub embed: Option<DiscordEmbed>,
}

impl serde::Serialize for DiscordMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("DiscordMessage", 4)?;

        // serialize each field except `embed`
        state.serialize_field("content", &self.content)?;
        state.serialize_field("username", &self.username)?;
        state.serialize_field("avatar_url", &self.avatar_url)?;

        // custom serialization for `embed` as `Vec<DiscordEmbed>`
        let embed_as_vec: Vec<DiscordEmbed> = self.embed.clone().into_iter().collect();
        state.serialize_field("embeds", &embed_as_vec)?;

        state.end()
    }
}

impl DiscordMessage {
    pub fn new() -> Self {
        DiscordMessage {
            content: None,
            username: None,
            avatar_url: None,
            embed: None,
        }
    }
}

impl Default for DiscordMessage {
    fn default() -> Self {
        let host_uri = crate::env_utils::get_host_uri();
        let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

        let mut message = Self::new();
        message.username = Some("TOTT".to_string());
        message.avatar_url = Some(avatar_url.to_string());
        message
    }
}

impl From<DiscordMessage> for reqwest::multipart::Form {
    fn from(val: DiscordMessage) -> Self {
        let mut form = reqwest::multipart::Form::new();

        if let Ok(payload_json) = serde_json::to_string(&val) {
            tracing::debug!("Payload JSON: {}", payload_json);
            form = form.text("payload_json", payload_json);
        }

        if let Some(embed) = &val.embed {
            if let Some(EmbedImage::Bytes(image)) = &embed.image {
                let image = image.clone();
                form = form.part(
                    "file1",
                    reqwest::multipart::Part::bytes(image.bytes).file_name(image.file_name.clone()),
                );
            }
        }

        form
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiscordEmbed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub fields: Option<Vec<EmbedField>>,
    pub image: Option<EmbedImage>,
    pub footer: Option<EmbedFooter>,
}

impl DiscordEmbed {
    pub fn new() -> Self {
        DiscordEmbed {
            title: None,
            description: None,
            fields: None,
            image: None,
            footer: None,
        }
    }

    pub fn title(&mut self, title: &str) -> &mut Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn description(&mut self, description: &str) -> &mut Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn field(&mut self, name: &str, value: &str, inline: bool) -> &mut Self {
        let field = EmbedField {
            name: name.to_string(),
            value: value.to_string(),
            inline: Some(inline),
        };
        if let Some(fields) = &mut self.fields {
            fields.push(field);
        } else {
            self.fields = Some(vec![field]);
        }
        self
    }

    pub fn image(&mut self, image: EmbedImage) -> &mut Self {
        self.image = Some(image);
        self
    }

    pub fn build(&mut self) -> Self {
        self.clone()
    }
}

impl Default for DiscordEmbed {
    fn default() -> Self {
        let host_uri = crate::env_utils::get_host_uri();
        let avatar_url = &format!("{}/assets/android-chrome-192x192.png", host_uri);

        let mut embed = Self::new();
        embed.footer = Some(EmbedFooter {
            text: "Powered by troyonthetrails.com".to_string(),
            icon_url: avatar_url.to_string(),
        });

        embed
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    pub inline: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub enum EmbedImage {
    Url(URLImageSource),
    Bytes(ByteImageSource),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct URLImageSource {
    url: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ByteImageSource {
    bytes: Vec<u8>,
    file_name: String,
}

impl serde::Serialize for EmbedImage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            EmbedImage::Url(url_image) => {
                let mut state = serializer.serialize_struct("URLImageSource", 1)?;
                state.serialize_field("url", &url_image.url)?;
                state.end()
            }
            EmbedImage::Bytes(byte_image) => {
                let mut state = serializer.serialize_struct("URLImageSource", 1)?;
                let url = format!("attachment://{}", byte_image.file_name);
                state.serialize_field("url", &url)?;
                state.end()
            }
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbedFooter {
    pub text: String,
    pub icon_url: String,
}

struct StringMessage(String);

impl From<StringMessage> for DiscordMessage {
    fn from(val: StringMessage) -> Self {
        let embed = DiscordEmbed::default().title(&val.0).build();
        DiscordMessage {
            embed: Some(embed),
            ..Default::default()
        }
    }
}

async fn send_webhook(message: impl Into<DiscordMessage>) {
    let message: DiscordMessage = message.into();

    let webhook_url = match std::env::var("DISCORD_WEBHOOK_URL") {
        Ok(url) => url,
        Err(_) => {
            tracing::debug!("No Discord webhook URL found, skipping");
            return;
        }
    };

    let client = reqwest::Client::builder()
        .build()
        .expect("Failed to build reqwest client");

    let request = client
        .request(reqwest::Method::POST, webhook_url)
        .multipart(message.into());

    match request.send().await {
        Ok(_) => {
            tracing::debug!("Successfully sent Discord webhook");
        }
        Err(e) => {
            tracing::error!("Failed to send Discord webhook: {}", e);
        }
    }
}

pub async fn send_starting_webhook(beacon_url: String) {
    send_webhook(OnTrailsNotification { beacon_url }).await;
}

pub async fn send_end_webhook(activity_id: Option<i64>) {
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
    let webhook_data: Option<WebhookData> = {
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

                let image: Option<WebhookImage> = {
                    let map_service_url = std::env::var("MAP_SERVICE_URL");
                    let polyline = match activity.map {
                        Some(map) => Some(map.summary_polyline),
                        None => None,
                    };
                    if let (Ok(map_service_url), Some(polyline)) = (map_service_url, polyline) {
                        match get_map_image(
                            map_service_url,
                            URLParams {
                                title: name.clone(),
                                polyline: Some(polyline),
                                duration: Some(activity.elapsed_time.to_string()),
                                distance: Some(activity.distance.to_string()),
                                elevation_gain: Some(activity.total_elevation_gain.to_string()),
                                average_speed: Some(activity.average_speed.to_string()),
                                top_speed: Some(activity.max_speed.to_string()),
                                as_image: Some(true),
                            },
                        )
                        .await
                        {
                            Ok(data) => Some(WebhookImage(data)),
                            Err(e) => {
                                tracing::error!("Failed to get map image: {}", e);
                                None
                            }
                        }
                    } else {
                        None
                    }
                };

                Some(WebhookData {
                    name,
                    distance,
                    total_elevation_gain,
                    average_speed,
                    max_speed,
                    image,
                })
            }
        }
    };

    send_webhook(OffTrailsNotification { webhook_data }).await;
}

async fn get_map_image(map_service_url: String, url_params: URLParams) -> anyhow::Result<Vec<u8>> {
    let client = reqwest::Client::builder().build()?;

    let response = client
        .request(reqwest::Method::GET, map_service_url)
        .query(&url_params)
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Err(anyhow::anyhow!("Recived status code 204"));
    }

    let bytes = response.bytes().await?;
    let bytes: Vec<u8> = bytes.iter().cloned().collect();

    if bytes.is_empty() {
        return Err(anyhow::anyhow!("Empty image data"));
    }

    Ok(bytes)
}

pub async fn send_discard_webhook() {
    send_webhook(StringMessage(
        "Troy has discarded the Strava activity".to_string(),
    ))
    .await;
}

// test
#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    #[tokio::test]
    async fn test_send_ending_webhook() {
        dotenv::dotenv().ok();

        let env_filter = tracing_subscriber::EnvFilter::builder()
            .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
            .from_env_lossy();
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();

        let _db = crate::db_service::get_db_service().await;

        let activity_id = 11982065575;
        send_end_webhook(Some(activity_id)).await;
    }
}
