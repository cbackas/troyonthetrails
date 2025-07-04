pub mod auth;
pub mod beacon;

use std::time::Duration;

use anyhow::Context;
use reqwest::{header, Response};
use tokio::{
    sync::OnceCell,
    time::{sleep, Instant},
};

use shared_lib::env_utils;
use shared_lib::structs::{Activity, StravaData, StravaDataCache};

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
        "https://www.strava.com/api/v3/athletes/{strava_user_id}/stats"
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
        "https://www.strava.com/api/v3/activities/{activity_id}"
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
