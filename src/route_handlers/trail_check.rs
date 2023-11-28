use std::sync::Arc;

use anyhow::Context;
use axum::extract::State;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::log::{debug, error};

use crate::AppState;

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TrailStatus {
    Open,
    Caution,
    Closed,
    Freeze,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct TrailSystem {
    id: u64,
    status: TrailStatus,
    name: String,
    city: String,
    state: String,
    facebook_url: String,
    lat: f64,
    lng: f64,
    total_distance: f64,
    description: String,
    pdf_map_url: String,
    video_url: Option<String>,
    external_url: String,
    status_description: String,
    directions_url: String,
}

pub async fn handler(
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl axum::response::IntoResponse {
    {
        let state = state.lock().await;
        if let Some(last_updated) = state.trail_data_last_updated {
            // if the trail data was updated less than 5 minutes ago, just use that
            if last_updated.elapsed().as_secs() < 300 {
                debug!("Using cached trail data");
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
    // update the cached trail data
    {
        let mut state = state.lock().await;
        state.trail_data = trail_data.clone();
        state.trail_data_last_updated = Some(tokio::time::Instant::now());
    }

    let template = TrailCheckTemplate { trails: trail_data };

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

    debug!("Fetched trail data from data source");

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

    let trail_systems: Vec<TrailSystem> =
        serde_json::from_str(json).context("Failed to parse trail data json")?;

    Ok(trail_systems)
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
