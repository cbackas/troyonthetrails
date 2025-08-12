pub mod auth;
pub mod beacon;

use std::time::Duration;

use anyhow::Context;
use reqwest::Url;
use reqwest::{header, Response};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};

use shared_lib::env_utils;
use shared_lib::strava_structs::{Activity, StravaData};

pub struct AthelteStatsCache {
    pub stats: StravaData,
    pub updated: Instant,
}

pub struct RidesCache {
    pub rides: Vec<Activity>,
    pub updated: Instant,
}

const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

async fn get_strava_data(url: String) -> anyhow::Result<Response> {
    let strava_token = auth::get_token()
        .await
        .context("Failed to get strava token")?;
    tracing::info!("Using Strava token: {}", strava_token.access_token);
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

pub async fn get_paginated_strava_data<T>(base_url: String) -> anyhow::Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let mut url = Url::parse(&base_url).context("Invalid base URL")?;

    {
        let mut query_pairs = url
            .query_pairs()
            .into_owned()
            .collect::<Vec<(String, String)>>();
        query_pairs.retain(|(k, _)| k != "page" && k != "per_page");
        url.query_pairs_mut().clear().extend_pairs(query_pairs);
    }

    let mut all_results = Vec::new();
    let per_page = 200;
    let mut page = 1;

    loop {
        {
            let mut qp = url
                .query_pairs()
                .into_owned()
                .collect::<Vec<(String, String)>>();
            qp.push(("per_page".into(), per_page.to_string()));
            qp.push(("page".into(), page.to_string()));

            let mut u = url.clone();
            u.query_pairs_mut().clear().extend_pairs(qp);
            url = u;
        }

        let resp = get_strava_data(url.to_string()).await?;
        let items: Vec<T> = resp
            .json()
            .await
            .context("Failed to deserialize Strava page")?;

        if items.is_empty() {
            break;
        }

        all_results.extend(items);
        page += 1;
    }

    Ok(all_results)
}

static CACHE_ATHLETE_STATS: LazyLock<Arc<Mutex<Option<AthelteStatsCache>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));
pub async fn get_athlete_stats() -> anyhow::Result<StravaData> {
    {
        if let Some(cached_stats) = &*CACHE_ATHLETE_STATS.lock().await {
            let now = Instant::now();
            let time_since_last_update = now - cached_stats.updated;
            if time_since_last_update.as_secs() < 60 * 5 {
                tracing::trace!("Using cached athlete stats");
                return Ok(cached_stats.stats.clone());
            }
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

        {
            let mut guard = CACHE_ATHLETE_STATS.lock().await;
            *guard = Some(AthelteStatsCache {
                stats: strava_data.clone(),
                updated: Instant::now(),
            });
        }

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

static CACHE_RIDES: LazyLock<Arc<Mutex<Option<RidesCache>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));
pub async fn get_all_activities() -> anyhow::Result<Vec<Activity>> {
    {
        if let Some(cached_rides) = &*CACHE_RIDES.lock().await {
            let now = Instant::now();
            let time_since_last_update = now - cached_rides.updated;
            if time_since_last_update.as_secs() < 60 * 5 {
                return Ok(cached_rides.rides.clone());
            }
        }
    }

    let activities: Vec<Activity> =
        get_paginated_strava_data("https://www.strava.com/api/v3/athlete/activities".to_string())
            .await
            .context("Failed to get paginated strava data")?
            .into_iter()
            .filter(|activity: &Activity| activity.type_field == "Ride")
            .collect();

    {
        let mut guard = CACHE_RIDES.lock().await;
        *guard = Some(RidesCache {
            rides: activities.clone(),
            updated: Instant::now(),
        });
    }

    Ok(activities)
}
