use core::fmt;

use chrono::{DateTime, TimeZone, Utc};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeaconData {
    pub streams: Streams,
    pub live_activity_id: i64,
    pub athlete_id: i64,
    pub update_time: EpochDateTime,
    pub utc_offset: i64,
    pub activity_type: i64,
    pub status: Status,
    pub stats: Stats,
    pub battery_level: i64,
    pub source_app: String,
    pub activity_id: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Streams {
    #[serde(default)]
    pub timestamp: Vec<i64>,
    #[serde(default)]
    pub latlng: Vec<Vec<f64>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub distance: f64,
    pub moving_time: i64,
    pub elapsed_time: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "i64", into = "i64")]
pub enum Status {
    Unknown,
    Active,
    AutoPaused,
    ManualPaused,
    Uploaded,
    Dicarded,
    NotStarted,
}

impl From<i64> for Status {
    fn from(value: i64) -> Self {
        match value {
            1 => Status::Active,
            2 => Status::AutoPaused,
            3 => Status::ManualPaused,
            4 => Status::Unknown,
            5 => Status::Uploaded,
            6 => Status::Dicarded,
            7 => Status::NotStarted,
            _ => Status::Unknown,
        }
    }
}

impl From<Status> for i64 {
    fn from(val: Status) -> Self {
        match val {
            Status::Active => 1,
            Status::AutoPaused => 2,
            Status::ManualPaused => 3,
            Status::Unknown => 4,
            Status::Uploaded => 5,
            Status::Dicarded => 6,
            Status::NotStarted => 7,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpochDateTime(DateTime<Utc>);

impl EpochDateTime {
    pub fn new(epoch: i64) -> Self {
        let thing = Utc.timestamp_opt(epoch, 0);
        EpochDateTime(thing.unwrap())
    }

    pub fn datetime(&self) -> &DateTime<Utc> {
        &self.0
    }
}

// Custom serialization for EpochDateTime
impl Serialize for EpochDateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(self.0.timestamp())
    }
}

// Custom deserialization for EpochDateTime
impl<'de> Deserialize<'de> for EpochDateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EpochDateTimeVisitor;

        impl<'de> Visitor<'de> for EpochDateTimeVisitor {
            type Value = EpochDateTime;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an integer representing an epoch timestamp")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(EpochDateTime::new(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(EpochDateTime::new(value as i64))
            }
        }

        deserializer.deserialize_i64(EpochDateTimeVisitor)
    }
}

pub async fn get_beacon_data(beacon_url: String) -> anyhow::Result<BeaconData> {
    let client = reqwest::Client::new();
    let resp = client
        .get(&beacon_url)
        .header("X-Requested-With", "XMLHttpRequest")
        .send()
        .await?;

    if resp.status().is_success() {
        let data: BeaconData = resp.json().await?;
        Ok(data)
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}
