use anyhow::Context;
use reqwest::{header, Response};
use serde::{Deserialize, Serialize};
use serde_json::{map::Values, Value};

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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StravaActivities {
    pub sessions: Vec<Activity>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub resource_state: i64,
    pub athlete: Athlete,
    pub name: String,
    pub distance: f64,
    pub moving_time: i64,
    pub elapsed_time: i64,
    pub total_elevation_gain: i64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub sport_type: String,
    pub workout_type: Value,
    pub id: i64,
    pub external_id: String,
    pub upload_id: f64,
    pub start_date: String,
    pub start_date_local: String,
    pub timezone: String,
    pub utc_offset: i64,
    pub start_latlng: Value,
    pub end_latlng: Value,
    pub location_city: Value,
    pub location_state: Value,
    pub location_country: String,
    pub achievement_count: i64,
    pub kudos_count: i64,
    pub comment_count: i64,
    pub athlete_count: i64,
    pub photo_count: i64,
    pub map: Map,
    pub trainer: bool,
    pub commute: bool,
    pub manual: bool,
    pub private: bool,
    pub flagged: bool,
    pub gear_id: String,
    pub from_accepted_tag: bool,
    pub average_speed: f64,
    pub max_speed: f64,
    pub average_cadence: f64,
    pub average_watts: f64,
    pub weighted_average_watts: i64,
    pub kilojoules: f64,
    pub device_watts: bool,
    pub has_heartrate: bool,
    pub average_heartrate: f64,
    pub max_heartrate: i64,
    pub max_watts: i64,
    pub pr_count: i64,
    pub total_photo_count: i64,
    pub has_kudoed: bool,
    pub suffer_score: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Athlete {
    pub id: i64,
    pub resource_state: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Map {
    pub id: String,
    pub summary_polyline: Value,
    pub resource_state: i64,
}

async fn get_strava_data(strava_token: String, url: String) -> anyhow::Result<Response> {
    let client = reqwest::Client::new();
    client
        .get(url)
        .header(header::AUTHORIZATION, format!("Bearer {}", strava_token))
        .send()
        .await
        .context("Failed to get strava data")
}

pub async fn get_athelete_stats(strava_token: String) -> anyhow::Result<StravaData> {
    let strava_user_id =
        std::env::var("STRAVA_USER_ID").context("STRAVA_USER_ID environment variable not found")?;

    let resp = get_strava_data(
        strava_token,
        format!(
            "https://www.strava.com/api/v3/athletes/{}/stats",
            strava_user_id
        ),
    )
    .await?;

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

pub async fn get_recent_activities(strava_token: String) -> anyhow::Result<StravaActivities> {
    let strava_user_id =
        std::env::var("STRAVA_USER_ID").context("STRAVA_USER_ID environment variable not found")?;

    let resp = get_strava_data(
        strava_token,
        "https://www.strava.com/api/v3/athlete/activities?per_page=3".to_string(),
    )
    .await?;

    if resp.status().is_success() {
        let text = resp.text().await.context("Failed to get strava data")?;

        let strava_data: StravaActivities =
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
