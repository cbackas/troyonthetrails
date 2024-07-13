use std::sync::Arc;

use anyhow::Context;
use axum::extract::State;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use tokio::sync::Mutex;
use tracing::log::error;
use tracing::{trace, warn};

use crate::AppState;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TrailStatus {
    Open,
    Caution,
    Closed,
    Freeze,
    Unknown,
}

// custom deserializer for TrailStatus
// basically allows for the Unknown variant to be used as a catchall
impl<'de> Deserialize<'de> for TrailStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TrailStatusVisitor;

        impl<'de> Visitor<'de> for TrailStatusVisitor {
            type Value = TrailStatus;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid trail status")
            }

            fn visit_str<E>(self, value: &str) -> Result<TrailStatus, E>
            where
                E: de::Error,
            {
                match value.to_lowercase().as_str() {
                    "open" => Ok(TrailStatus::Open),
                    "caution" => Ok(TrailStatus::Caution),
                    "closed" => Ok(TrailStatus::Closed),
                    "freeze" => Ok(TrailStatus::Freeze),
                    _ => {
                        warn!("Unknown trail status: {}", value);
                        Ok(TrailStatus::Unknown)
                    }
                }
            }
        }

        deserializer.deserialize_str(TrailStatusVisitor)
    }
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct TrailSystem {
    id: u64,
    status: TrailStatus,
    name: String,
    city: String,
    state: String,
    facebook_url: Option<String>,
    lat: f64,
    lng: f64,
    total_distance: f64,
    description: Option<String>,
    pdf_map_url: Option<String>,
    video_url: Option<String>,
    external_url: Option<String>,
    status_description: String,
    directions_url: Option<String>,
}

pub async fn handler(
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl axum::response::IntoResponse {
    {
        let state = state.lock().await;
        if let Some(last_updated) = state.trail_data_last_updated {
            // if the trail data was updated less than 5 minutes ago, just use that
            if last_updated.elapsed().as_secs() < 300 {
                trace!("Using cached trail data");
                let template = TrailCheckTemplate {
                    trails: state.trail_data.clone(),
                };
                return super::html_template::HtmlTemplate(template);
            }
        }
    }

    let trail_data: Vec<TrailSystem> = match get_trail_html().await {
        Ok(html) => match extract_trail_data(html) {
            Ok(data) => data,
            Err(err) => {
                error!("Failed to extract trail data: {}", err);
                vec![]
            }
        },
        Err(err) => {
            error!("Failed to get trail HTML: {}", err);
            vec![]
        }
    };

    let trail_data = sort_trail_data(trail_data);

    let template = TrailCheckTemplate {
        trails: trail_data.clone(),
    };

    // update the cached trail data
    {
        let mut state = state.lock().await;
        state.trail_data = trail_data;
        state.trail_data_last_updated = Some(tokio::time::Instant::now());
    }

    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "components/trail_check.html")]
struct TrailCheckTemplate {
    pub trails: Vec<TrailSystem>,
}

async fn get_trail_html() -> anyhow::Result<String> {
    let url =
        std::env::var("TRAIL_DATA_URL").context("TRAIL_DATA_URL environment variable not found")?;

    let resp = reqwest::get(url)
        .await
        .context("Failed to get HTML from data source")?;
    let html = resp.text().await.context("Couldn't find html body")?;

    trace!("Fetched trail data from data source");

    Ok(html)
}

fn extract_trail_data(html: String) -> anyhow::Result<Vec<TrailSystem>> {
    let start_tag = "var trail_systems = ";
    let end_tag = ";</script>";

    let start = html
        .find(start_tag)
        .ok_or(anyhow::anyhow!("Start tag not found"))?
        + start_tag.len();
    let end = html[start..]
        .find(end_tag)
        .ok_or(anyhow::anyhow!("End tag not found"))?
        + start;

    let json = &html[start..end];

    let trail_systems = serde_json::from_str(json);
    match trail_systems {
        Ok(trail_systems) => Ok(trail_systems),
        Err(err) => Err(err.into()),
    }
}

fn sort_trail_data(trail_data: Vec<TrailSystem>) -> Vec<TrailSystem> {
    let static_lat = match std::env::var("HOME_LAT")
        .map_err(|e| e.to_string())
        .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
    {
        Ok(lat) => lat,
        Err(err) => {
            error!("Failed to parse HOME_LAT environment variable: {}", err);
            return trail_data;
        }
    };

    let static_lng = match std::env::var("HOME_LNG")
        .map_err(|e| e.to_string())
        .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
    {
        Ok(lng) => lng,
        Err(err) => {
            error!("Failed to parse HOME_LNG environment variable: {}", err);
            return trail_data;
        }
    };

    let mut sorted_data = trail_data;
    sorted_data.sort_by(|a, b| {
        let distance_a = ((a.lat - static_lat).powi(2) + (a.lng - static_lng).powi(2)).sqrt();
        let distance_b = ((b.lat - static_lat).powi(2) + (b.lng - static_lng).powi(2)).sqrt();
        distance_a
            .partial_cmp(&distance_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    sorted_data
}
