use serde::{Deserialize, Serialize};
use tokio::time::Instant;

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct StravaTokenResponse {
    pub token_type: String,
    pub expires_at: u64,
    pub expires_in: u64,
    pub refresh_token: String,
    pub access_token: String,
    pub athlete: Option<Athlete>,
}

impl From<StravaTokenResponse> for TokenData {
    fn from(val: StravaTokenResponse) -> Self {
        TokenData {
            expires_at: val.expires_at,
            access_token: val.access_token,
            refresh_token: val.refresh_token,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenData {
    pub expires_at: u64,
    pub access_token: String,
    pub refresh_token: String,
}

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
    pub start_latlng: Option<Vec<f64>>,
    pub end_latlng: Option<Vec<f64>>,
    #[serde(flatten)]
    other: serde_json::Value, // catch-all
}

impl TryFrom<Activity> for geo::Point {
    type Error = &'static str;

    fn try_from(activity: Activity) -> Result<Self, Self::Error> {
        if let Some(start_latlng) = activity.start_latlng {
            if start_latlng.len() == 2 {
                let lat = start_latlng[0];
                let lng = start_latlng[1];
                if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
                    return Err("Invalid coordinates");
                }
                return Ok(geo::Point::new(start_latlng[1], start_latlng[0]));
            }
        }
        Err("Invalid coordinates")
    }
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
