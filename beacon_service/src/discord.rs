use serde::ser::SerializeStruct;

use map_service::{DefaultColor, MapImage, TextAlignment, TextOptions};
use shared_lib::structs::Activity;

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
        let host_uri = shared_lib::env_utils::get_host_uri();
        let avatar_url = &format!("{host_uri}/assets/android-chrome-192x192.png");

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
        let host_uri = shared_lib::env_utils::get_host_uri();
        let avatar_url = &format!("{host_uri}/assets/android-chrome-192x192.png");

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
        Some(activity_id) => match strava_service::get_activity(activity_id).await {
            Ok(activity) => Some(activity),
            Err(e) => {
                tracing::error!("Failed to get last activity: {:?}", e);
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
                let distance = shared_lib::utils::meters_to_miles(activity.distance, false);
                let total_elevation_gain =
                    shared_lib::utils::meters_to_feet(activity.total_elevation_gain, true);
                let average_speed = shared_lib::utils::mps_to_miph(activity.average_speed, false);
                let max_speed = shared_lib::utils::mps_to_miph(activity.max_speed, false);

                let image: Option<WebhookImage> = {
                    let polyline = match activity.map {
                        Some(map) => map.summary_polyline,
                        None => return,
                    };

                    match get_map_image(
                        polyline,
                        &name,
                        activity.elapsed_time,
                        distance,
                        total_elevation_gain,
                        average_speed,
                        max_speed,
                    )
                    .await
                    {
                        Ok(data) => Some(WebhookImage(data)),
                        Err(e) => {
                            tracing::error!("Failed to get map image: {:?}", e);
                            None
                        }
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

async fn get_map_image(
    polyline: String,
    title: &Option<String>,
    duration: i64,
    distance: f64,
    elevation_gain: f64,
    average_speed: f64,
    top_speed: f64,
) -> anyhow::Result<Vec<u8>> {
    const TITLE_ROW_HEIGHT: f32 = 50.0;
    const DATA_ROW_HEIGHT: f32 = 36.0;

    let mut map_image = MapImage::new(&polyline)?;

    if let Some(title) = &title {
        map_image
            .add_text(
                title.to_uppercase().as_str(),
                TextOptions {
                    color: DefaultColor::White,
                    font_size: TITLE_ROW_HEIGHT,
                    alignment: TextAlignment::Center,
                },
            )
            .add_spacer();
    }

    let duration = shared_lib::utils::minutes_to_human_readable(duration);
    map_image
        .add_text(
            format!("{duration} ride").as_str(),
            TextOptions {
                color: DefaultColor::White,
                font_size: DATA_ROW_HEIGHT,
                alignment: TextAlignment::Center,
            },
        )
        .add_spacer();

    map_image.add_text_with_svg(
        format!("Rode {distance} miles").as_str(),
        TextOptions {
            color: DefaultColor::White,
            font_size: DATA_ROW_HEIGHT,
            alignment: TextAlignment::Left,
        },
        include_bytes!("../assets/measure-2-svgrepo-com.svg"),
    );

    map_image.add_text_with_svg(
        format!("Climbed {elevation_gain} feet").as_str(),
        TextOptions {
            color: DefaultColor::White,
            font_size: DATA_ROW_HEIGHT,
            alignment: TextAlignment::Left,
        },
        include_bytes!("../assets/climb-svgrepo-com.svg"),
    );

    map_image.add_text_with_svg(
        format!("Average speed of {average_speed:.1} mph").as_str(),
        TextOptions {
            color: DefaultColor::White,
            font_size: DATA_ROW_HEIGHT,
            alignment: TextAlignment::Left,
        },
        include_bytes!("../assets/speedometer-svgrepo-com.svg"),
    );

    map_image.add_text_with_svg(
        format!("Top speed of {top_speed:.1} mph").as_str(),
        TextOptions {
            color: DefaultColor::White,
            font_size: DATA_ROW_HEIGHT,
            alignment: TextAlignment::Left,
        },
        include_bytes!("../assets/lightning-charge-svgrepo-com.svg"),
    );

    let map_image = map_image.encode_png()?;

    Ok(map_image)
}

pub async fn send_discard_webhook() {
    send_webhook(StringMessage(
        "Troy has discarded the Strava activity".to_string(),
    ))
    .await;
}

// test
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
//
//     #[tokio::test]
//     async fn test_send_ending_webhook() {
//         dotenv::dotenv().ok();
//
//         let env_filter = tracing_subscriber::EnvFilter::builder()
//             .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
//             .from_env_lossy();
//         tracing_subscriber::registry()
//             .with(env_filter)
//             .with(tracing_subscriber::fmt::layer())
//             .init();
//
//         let _db = db_service::get_db_service().await;
//
//         let activity_id = 13865285076;
//         send_end_webhook(Some(activity_id)).await;
//     }
// }
