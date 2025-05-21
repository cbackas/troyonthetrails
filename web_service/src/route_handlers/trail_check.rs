use std::sync::Arc;

use anyhow::Context;
use axum::extract::State;
use serde::{Deserialize, Deserializer};
use tokio::sync::Mutex;
use tracing::log::error;

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
        // Use serde_json::Value to capture any type
        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::String(s) => match s.to_lowercase().as_str() {
                "open" => Ok(TrailStatus::Open),
                "caution" => Ok(TrailStatus::Caution),
                "closed" => Ok(TrailStatus::Closed),
                "freeze" => Ok(TrailStatus::Freeze),
                other => {
                    tracing::warn!("Unknown trail status: {}", other);
                    Ok(TrailStatus::Unknown)
                }
            },
            serde_json::Value::Null => {
                tracing::warn!("Null trail status");
                Ok(TrailStatus::Unknown)
            }
            _ => {
                tracing::warn!("Invalid trail status type: {:?}", value);
                Ok(TrailStatus::Unknown)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictedStatus {
    pub status: TrailStatus,
    pub confidence: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
    #[serde(rename = "status_description")]
    pub status_description: String,
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
    latest_status_update_at: Option<String>,
    predicted_status: Option<PredictedStatus>,
}

pub async fn handler(
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl axum::response::IntoResponse {
    {
        let state = state.lock().await;
        if let Some(last_updated) = state.trail_data_last_updated {
            // if the trail data was updated less than 5 minutes ago, just use that
            if last_updated.elapsed().as_secs() < 300 {
                tracing::trace!("Using cached trail data");
                let template = TrailCheckTemplate {
                    trails: state.trail_data.clone(),
                };
                return super::html_template::HtmlTemplate(template);
            }
        }
    }

    let trail_data = match crate::trail_lib::get_trail_data().await {
        Ok(data) => sort_trail_data(data),
        Err(err) => {
            error!("Failed to get trail data: {}", err);
            return super::html_template::HtmlTemplate(TrailCheckTemplate { trails: vec![] });
        }
    };

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

impl TrailCheckTemplate {
    pub fn format_time_ago(&self, datetime_str: &Option<String>) -> String {
        match datetime_str {
            Some(dt) => crate::utils::utc_to_time_ago_human_readable(dt),
            None => Default::default(),
        }
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
