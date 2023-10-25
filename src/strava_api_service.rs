use std::{
    env, fs,
    io::{self, ErrorKind},
    path::PathBuf,
    time::Duration,
};

use anyhow::Context;
use lazy_static::lazy_static;
use reqwest::{header, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    sync::Mutex,
    time::{sleep, Instant},
};
use tracing::debug;

use crate::env_utils;

#[derive(Deserialize, Debug, Clone)]
pub struct StravaTotals {
    pub count: u32,
    pub distance: f64,
    pub moving_time: u64,
    pub elapsed_time: u64,
    pub elevation_gain: f64,
    pub achievement_count: Option<u32>,
}

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
    pub resource_state: i64,
    pub athlete: Athlete,
    pub name: String,
    pub distance: f64,
    pub moving_time: i64,
    pub elapsed_time: i64,
    pub total_elevation_gain: f64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub sport_type: String,
    pub workout_type: Option<i64>,
    pub id: i64,
    pub start_date: String,
    pub start_date_local: String,
    pub timezone: String,
    pub utc_offset: f64, // This field should be a floating-point type
    pub start_latlng: Vec<f64>,
    pub end_latlng: Vec<f64>,
    pub location_city: Option<String>,
    pub location_state: Option<String>,
    pub location_country: String,
    pub achievement_count: i64,
    pub kudos_count: i64,
    pub comment_count: i64,
    pub athlete_count: i64,
    pub photo_count: i64,
    pub map: Value,
    pub trainer: bool,
    pub commute: bool,
    pub manual: bool,
    pub private: bool,
    pub flagged: bool,
    pub gear_id: Option<String>,
    pub from_accepted_tag: bool,
    pub average_speed: f64,
    pub max_speed: f64,
    pub has_heartrate: bool,
    pub heartrate_opt_out: bool,
    pub display_hide_heartrate_option: bool,
    pub elev_high: f64,
    pub elev_low: f64,
    pub upload_id: i64, // This field should be an integer type
    pub upload_id_str: String,
    pub external_id: String,
    pub pr_count: i64,
    pub total_photo_count: i64,
    pub has_kudoed: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Athlete {
    pub id: u64,
    #[serde(flatten)]
    other: serde_json::Value, // This will capture all other fields within 'athlete' as a JSON value.
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

const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

lazy_static! {
    pub static ref API_SERVICE: Mutex<StravaAPIService> = Mutex::new(StravaAPIService::new());
}

pub struct StravaAPIService {
    pub token_data: Option<TokenData>,
    pub strava_user_id: Option<String>,
    pub strava_athlete_stats: Option<StravaData>,
    pub strava_athlete_stats_updated: Option<Instant>,
}

impl StravaAPIService {
    pub fn new() -> Self {
        let token_data = match read_token_data_from_file() {
            Ok(token_data) => Some(token_data),
            Err(_) => None,
        };
        let strava_user_id = env_utils::get_strava_user_id();

        Self {
            token_data,
            strava_user_id,
            strava_athlete_stats: None,
            strava_athlete_stats_updated: None,
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
                let strava_user_id = match self.strava_user_id {
                    Some(ref strava_user_id) => strava_user_id,
                    None => {
                        return Err(anyhow::anyhow!(
                            "Successfully authenticated Strava user but no STRAVA_USER_ID defined"
                        ))
                    }
                };
                if athlete.id.to_string().as_str() != strava_user_id {
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

    async fn refresh_strava_token(&mut self) -> anyhow::Result<()> {
        let refresh_token = self
            .token_data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No strava token found, please authenticate"))?
            .refresh_token
            .clone();

        self.get_token_from_refresh(refresh_token)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to refresh strava token: {}", e.to_string()))
    }

    async fn get_valid_strava_token(&mut self) -> anyhow::Result<TokenData> {
        if let Some(token_data) = &self.token_data {
            if token_data.expires_at >= chrono::Utc::now().timestamp() as u64 {
                return Ok(token_data.clone());
            }
        }

        self.refresh_strava_token().await?;

        match &self.token_data {
            Some(token_data) => Ok(token_data.clone()),
            None => Err(anyhow::anyhow!(
                "No strava token found, please authenticate"
            )),
        }
    }

    async fn get_strava_data(&mut self, url: String) -> anyhow::Result<Response> {
        let strava_token = self.get_valid_strava_token().await?;
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

    /// Returns the cached athlete stats if they are less 5 minutes old
    fn get_cached_athlete_stats(&mut self) -> Option<StravaData> {
        let now = Instant::now();
        if let Some(strava_athlete_stats_updated) = self.strava_athlete_stats_updated {
            if now.duration_since(strava_athlete_stats_updated).as_secs() < 60 * 5 {
                return self.strava_athlete_stats.clone();
            }
        }
        None
    }

    pub async fn get_athlete_stats(&mut self) -> anyhow::Result<StravaData> {
        if let Some(strava_athlete_stats) = self.get_cached_athlete_stats() {
            debug!("Using cached athlete stats");
            return Ok(strava_athlete_stats);
        }

        let strava_user_id = match self.strava_user_id {
            Some(ref strava_user_id) => strava_user_id,
            None => {
                return Err(anyhow::anyhow!(
                    "No strava user id found, please authenticate"
                ))
            }
        };

        debug!("Fetching new athlete stats");
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

            self.strava_athlete_stats = Some(strava_data.clone());
            self.strava_athlete_stats_updated = Some(Instant::now());

            Ok(strava_data)
        } else {
            Err(anyhow::anyhow!(
                "Received a non-success status code {}: {}",
                resp.status(),
                resp.text().await.unwrap_or("Unknown error".to_string())
            ))
        }
    }

    pub async fn get_recent_activity(&mut self) -> anyhow::Result<Activity> {
        let resp = self
            .get_strava_data(
                "https://www.strava.com/api/v3/athlete/activities?per_page=1".to_string(),
            )
            .await?;

        if resp.status().is_success() {
            let text = resp.text().await.context("Failed to get strava data")?;

            let strava_data: Vec<Activity> =
                serde_json::from_str(&text).context("Failed to deserialize JSON")?;
            let activity = match strava_data.first() {
                Some(activity) => Ok(activity.clone()),
                None => Err(anyhow::anyhow!("No activities found")),
            }?;

            let now = chrono::Utc::now();
            let last_activity_time = chrono::DateTime::parse_from_rfc3339(&activity.start_date)
                .unwrap()
                .with_timezone(&chrono::Utc);
            let time_since_last_activity = now - last_activity_time;
            if time_since_last_activity.num_hours() < 4 {
                Ok(activity)
            } else {
                Err(anyhow::anyhow!("No activities found"))
            }
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
