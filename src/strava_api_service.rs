use std::{
    env, fs,
    io::{self, ErrorKind},
    path::PathBuf,
};

use anyhow::Context;
use lazy_static::lazy_static;
use reqwest::{header, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::debug;

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
    pub id: u64,
    #[serde(flatten)]
    other: serde_json::Value, // This will capture all other fields within 'athlete' as a JSON value.
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Map {
    pub id: String,
    pub summary_polyline: Value,
    pub resource_state: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenData {
    pub expires_at: u64,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StravaTokenResponse {
    pub token_type: String,
    pub expires_at: u64,
    pub expires_in: u64,
    pub refresh_token: String,
    pub access_token: String,
    pub athlete: Option<Athlete>,
}

lazy_static! {
    pub static ref API_SERVICE: Mutex<StravaAPIService> = Mutex::new(StravaAPIService::new());
}

pub struct StravaAPIService {
    pub token_data: Option<TokenData>,
    pub strava_user_id: String,
}

impl StravaAPIService {
    pub fn new() -> Self {
        let token_data = match read_token_data_from_file() {
            Ok(token_data) => Some(token_data),
            Err(_) => None,
        };
        Self {
            token_data,
            strava_user_id: std::env::var("STRAVA_USER_ID").unwrap(),
        }
    }

    fn write_token_data_to_file(&mut self) -> io::Result<()> {
        let token_data = match &self.token_data {
            Some(token_data) => token_data,
            None => {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "No token data to write to file",
                ))
            }
        };

        let path = get_token_cache_path();
        let json_data = serde_json::to_string_pretty(&token_data)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        fs::write(path, json_data)?;
        debug!("Wrote token data to file");
        Ok(())
    }

    pub async fn get_token_from_code(&mut self, code: String) -> anyhow::Result<()> {
        let client_id = std::env::var("STRAVA_CLIENT_ID")
            .context("STRAVA_CLIENT_ID environment variable not found")?;
        let client_secret = std::env::var("STRAVA_CLIENT_SECRET")
            .context("STRAVA_CLIENT_SECRET environment variable not found")?;

        debug!("Fetching new strava token using OAuth flow");

        let client = reqwest::Client::new();
        let resp = client
            .post("https://www.strava.com/api/v3/oauth/token")
            .query(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("code", code),
                ("grant_type", "authorization_code".to_string()),
            ])
            .send()
            .await
            .context("Failed to get token from strava")?;

        if resp.status().is_success() {
            let strava_data = resp.text().await;
            let strava_data: StravaTokenResponse = serde_json::from_str(&strava_data.unwrap())
                .context("Failed to deserialize JSON")?;

            // if strava_data has an athlete then compare the id to the one in the env var
            if let Some(athlete) = strava_data.clone().athlete {
                let strava_user_id = std::env::var("STRAVA_USER_ID")
                    .context("STRAVA_USER_ID environment variable not found")?;
                if athlete.id.to_string() != strava_user_id {
                    return Err(anyhow::anyhow!(
                    "Successfully authenticated Strava user but the user id does not match the defined STRAVA_USER_ID"
                ));
                }
            }

            let strava_data = strava_data_to_token_data(strava_data);
            self.token_data = Some(strava_data);
            match self.write_token_data_to_file() {
                Ok(_) => {}
                Err(e) => debug!("Failed to write token data to file: {}", e),
            };

            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Received a non-success status code {}: {}",
                resp.status(),
                resp.text().await.unwrap_or("Unknown error".to_string())
            ))
        }
    }

    async fn get_token_from_refresh(&mut self, refresh_token: String) -> anyhow::Result<()> {
        let client_id = std::env::var("STRAVA_CLIENT_ID")
            .context("STRAVA_CLIENT_ID environment variable not found")?;
        let client_secret = std::env::var("STRAVA_CLIENT_SECRET")
            .context("STRAVA_CLIENT_SECRET environment variable not found")?;

        debug!("Fetching new strava token using refresh token");

        let client = reqwest::Client::new();
        let resp = client
            .post("https://www.strava.com/oauth/token")
            .query(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token".to_string()),
            ])
            .send()
            .await
            .context("Failed to refresh Strava token")?;

        if resp.status().is_success() {
            let strava_data = resp.text().await;
            let strava_data: StravaTokenResponse = serde_json::from_str(&strava_data.unwrap())
                .context("Failed to deserialize JSON")?;
            let strava_data = strava_data_to_token_data(strava_data);
            self.token_data = Some(strava_data);
            match self.write_token_data_to_file() {
                Ok(_) => {}
                Err(e) => debug!("Failed to write token data to file: {}", e),
            };

            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Received a non-success status code {}: {}",
                resp.status(),
                resp.text().await.unwrap_or("Unknown error".to_string())
            ))
        }
    }

    async fn get_strava_data(&mut self, url: String) -> anyhow::Result<Response> {
        let mut strava_token = match self.token_data {
            Some(ref token_data) => token_data,
            None => {
                return Err(anyhow::anyhow!(
                    "No strava token found, please authenticate"
                ))
            }
        };
        if strava_token.expires_at < chrono::Utc::now().timestamp() as u64 {
            match self
                .get_token_from_refresh(strava_token.refresh_token.clone())
                .await
            {
                Ok(()) => {
                    strava_token = match self.token_data {
                        Some(ref token_data) => token_data,
                        None => {
                            return Err(anyhow::anyhow!(
                                "No strava token found, please authenticate"
                            ))
                        }
                    };
                }

                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to refresh strava token: {}",
                        e.to_string()
                    ))
                }
            };
        }

        let client = reqwest::Client::new();
        client
            .get(url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", strava_token.access_token),
            )
            .send()
            .await
            .context("Failed to get strava data")
    }

    pub async fn get_athelete_stats(&mut self) -> anyhow::Result<StravaData> {
        let strava_user_id = std::env::var("STRAVA_USER_ID")
            .context("STRAVA_USER_ID environment variable not found")?;

        let resp = self
            .get_strava_data(format!(
                "https://www.strava.com/api/v3/athletes/{}/stats",
                strava_user_id
            ))
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

    pub async fn get_last_activity(&mut self) -> anyhow::Result<StravaActivities> {
        let resp = self
            .get_strava_data(
                "https://www.strava.com/api/v3/athlete/activities?per_page=1".to_string(),
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
}

fn get_token_cache_path() -> PathBuf {
    let base_path = env::var("TOKEN_DATA_PATH").unwrap_or_else(|_| "/data".to_string());
    PathBuf::from(base_path).join(".strava_auth.json")
}

fn read_token_data_from_file() -> io::Result<TokenData> {
    let path = get_token_cache_path();
    let file_content = fs::read_to_string(path)?;
    let token_data = serde_json::from_str(&file_content)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    Ok(token_data)
}

fn strava_data_to_token_data(strava_data: StravaTokenResponse) -> TokenData {
    TokenData {
        expires_at: strava_data.expires_at,
        access_token: strava_data.access_token,
        refresh_token: strava_data.refresh_token,
    }
}
