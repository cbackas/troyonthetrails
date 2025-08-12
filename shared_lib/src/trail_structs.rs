use serde::{Deserialize, Deserializer};

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
    pub id: u64,
    pub status: TrailStatus,
    pub name: String,
    pub city: String,
    pub state: String,
    pub facebook_url: Option<String>,
    pub lat: f64,
    pub lng: f64,
    pub total_distance: f64,
    pub description: Option<String>,
    pub pdf_map_url: Option<String>,
    pub video_url: Option<String>,
    pub external_url: Option<String>,
    pub status_description: String,
    pub directions_url: Option<String>,
    pub latest_status_update_at: Option<String>,
    pub predicted_status: Option<PredictedStatus>,
    pub stats: Option<TrailStatsDisplay>,
}

impl TryFrom<TrailSystem> for geo::Point {
    type Error = &'static str;

    fn try_from(trail: TrailSystem) -> Result<Self, Self::Error> {
        if !(-90.0..=90.0).contains(&trail.lat) || !(-180.0..=180.0).contains(&trail.lng) {
            return Err("Invalid coordinates");
        }
        Ok(geo::Point::new(trail.lng, trail.lat))
    }
}

#[derive(Default, Debug, Clone, Copy, Eq)]
pub struct TrailStats {
    pub id: u64,
    pub rides: i32,
    pub achievement_count: i64,
    pub total_moving_time: i64,
}

impl PartialEq for TrailStats {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.rides == other.rides
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct TrailStatsDisplay {
    pub id: u64,
    pub rides: String,
    pub achievement_count: i64,
    pub total_moving_time: String,
}

impl From<TrailStats> for TrailStatsDisplay {
    fn from(stats: TrailStats) -> Self {
        let rides = match stats.rides {
            1 => "1 time".to_string(),
            _ => format!("{} times", stats.rides),
        };

        let total_moving_time = match stats.total_moving_time {
            0 => "never".to_string(),
            elapsed => {
                let hours = (elapsed as f64 / 3600.0 * 2.0).round() / 2.0;
                if hours.fract() == 0.0 {
                    format!("{hours:.0}h")
                } else {
                    format!("{hours:.1}h")
                }
            }
        };

        TrailStatsDisplay {
            id: stats.id,
            rides,
            achievement_count: stats.achievement_count,
            total_moving_time,
        }
    }
}
