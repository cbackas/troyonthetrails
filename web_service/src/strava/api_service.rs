use std::time::Duration;

use serde::{Deserialize, Serialize};

use anyhow::Context;
use reqwest::{header, Response};
use tokio::{
    sync::OnceCell,
    time::{sleep, Instant},
};

use crate::env_utils;

use super::auth;

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct StravaTotals {
    pub count: u32,
    pub distance: f64,
    pub moving_time: f64,
    pub elapsed_time: f64,
    pub elevation_gain: f64,
    pub achievement_count: Option<u32>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct StravaData {
    pub biggest_ride_distance: f64,
    pub biggest_climb_elevation_gain: Option<f64>,
    pub recent_ride_totals: StravaTotals,
    pub all_ride_totals: StravaTotals,
    pub recent_run_totals: StravaTotals,
    pub all_run_totals: StravaTotals,
    pub recent_swim_totals: StravaTotals,
    pub all_swim_totals: StravaTotals,
    pub ytd_ride_totals: StravaTotals,
    pub ytd_run_totals: StravaTotals,
    pub ytd_swim_totals: StravaTotals,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub id: i64,
    pub resource_state: i64,
    pub athlete: Athlete,
    pub name: String,
    pub distance: f64,
    pub moving_time: i64,
    pub elapsed_time: i64,
    pub total_elevation_gain: f64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub achievement_count: i64,
    pub map: Option<Map>,
    pub average_speed: f64,
    pub max_speed: f64,
    pub elev_high: f64,
    pub elev_low: f64,
    #[serde(flatten)]
    other: serde_json::Value, // catch-all
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Athlete {
    pub id: u64,
    #[serde(flatten)]
    other: serde_json::Value, // catch-all
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Map {
    pub id: String,
    pub polyline: Option<String>,
    pub summary_polyline: String,
    pub resource_state: i64,
}

pub struct StravaDataCache {
    pub strava_athlete_stats: StravaData,
    pub strava_athlete_stats_updated: Instant,
}
static CACHED_DATA: OnceCell<StravaDataCache> = OnceCell::const_new();

const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

async fn get_strava_data(url: String) -> anyhow::Result<Response> {
    let strava_token = auth::get_token().await.expect("No token found");
    let client = reqwest::Client::new();

    for retry in 0..MAX_RETRIES {
        let response = client
            .get(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", strava_token.access_token),
            )
            .send()
            .await
            .context("Failed to send request")?;

        if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Ok(response);
        }

        let backoff_time = INITIAL_BACKOFF * 2u32.pow(retry);
        sleep(backoff_time).await;
    }

    Err(anyhow::anyhow!("Exceeded maximum retries"))
}

pub async fn get_athlete_stats() -> anyhow::Result<StravaData> {
    // return cached data if it's less than 5 minutes old
    let cached_stats = CACHED_DATA.get();
    if let Some(cached_stats) = cached_stats {
        let now = Instant::now();
        let time_since_last_update = now - cached_stats.strava_athlete_stats_updated;
        if time_since_last_update.as_secs() < 60 * 5 {
            return Ok(cached_stats.strava_athlete_stats.clone());
        }
    }

    tracing::trace!("Fetching new athlete stats");
    let strava_user_id = env_utils::get_strava_user_id().expect("No strava user id found");
    let resp = get_strava_data(format!(
        "https://www.strava.com/api/v3/athletes/{}/stats",
        strava_user_id
    ))
    .await?;

    if resp.status().is_success() {
        let text = resp.text().await.context("Failed to get strava data")?;

        let strava_data: StravaData =
            serde_json::from_str(&text).context("Failed to deserialize JSON")?;

        let _ = CACHED_DATA.set(StravaDataCache {
            strava_athlete_stats: strava_data.clone(),
            strava_athlete_stats_updated: Instant::now(),
        });

        Ok(strava_data)
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}

pub async fn get_activity(activity_id: i64) -> anyhow::Result<Activity> {
    let resp = get_strava_data(format!(
        "https://www.strava.com/api/v3/activities/{}",
        activity_id
    ))
    .await?;

    if resp.status().is_success() {
        let text = resp.text().await.context("Failed to get strava data")?;

        let activity: Activity =
            serde_json::from_str(&text).context("Failed to deserialize JSON")?;

        Ok(activity)
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}
