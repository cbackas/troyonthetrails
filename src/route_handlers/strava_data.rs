use std::sync::Arc;

use anyhow::Context;
use axum::extract::State;
use axum::response::IntoResponse;
use reqwest::header;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::error;

use crate::strava_token_utils::{get_token_from_refresh, TokenData};
use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct StravaTotals {
    pub count: u32,
    pub distance: f64,
    pub moving_time: u64,
    pub elapsed_time: u64,
    pub elevation_gain: f64,
    pub achievement_count: Option<u32>,
}

#[derive(Deserialize, Debug)]
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

pub async fn handler(app_state: State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let mut app_state = app_state.lock().await;
    let mut strava_token: TokenData = match &app_state.strava_token {
        Some(token) => token.clone(),
        None => {
            return axum::http::status::StatusCode::UNAUTHORIZED.into_response();
        }
    };

    if strava_token.expires_at < chrono::Utc::now().timestamp() as u64 {
        strava_token = match get_token_from_refresh(strava_token.refresh_token.clone()).await {
            Ok(token) => {
                app_state.strava_token = Some(token.clone());
                token
            }

            Err(err) => {
                error!("Failed to get strava token: {}", err);
                return axum::http::status::StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };
    }

    let strava_data = match get_strava_data(strava_token.access_token.clone()).await {
        Ok(data) => data,
        Err(err) => {
            error!("Failed to get strava data: {}", err);
            return axum::http::status::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    super::html_template::HtmlTemplate(StravaDataTemplate {
        total_rides: strava_data.all_ride_totals.count,
        total_distance: strava_data.all_ride_totals.distance / 1000.0,
        total_elevation_gain: strava_data.all_ride_totals.elevation_gain,
        longest_ride: strava_data.biggest_ride_distance / 1000.0,
    })
    .into_response()
}

#[derive(askama::Template)]
#[template(path = "components/strava_data.html")]
struct StravaDataTemplate {
    total_rides: u32,
    total_distance: f64,
    total_elevation_gain: f64,
    longest_ride: f64,
}

async fn get_strava_data(strava_token: String) -> anyhow::Result<StravaData> {
    let strava_user_id =
        std::env::var("STRAVA_USER_ID").context("STRAVA_USER_ID environment variable not found")?;

    let url = format!(
        "https://www.strava.com/api/v3/athletes/{}/stats",
        strava_user_id
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header(header::AUTHORIZATION, format!("Bearer {}", strava_token))
        .send()
        .await
        .context("Failed to get strava data")?;

    if resp.status().is_success() {
        let text = resp.text().await.context("Failed to get strava data")?;

        let strava_data: StravaData =
            serde_json::from_str(&text).context("Failed to deserialize JSON")?;
        Ok(strava_data)
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}
