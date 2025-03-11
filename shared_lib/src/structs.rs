use crate::utils;

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct URLParams {
    pub title: Option<String>,
    pub polyline: Option<String>,
    pub duration: Option<i64>,
    pub distance: Option<f64>,
    pub elevation_gain: Option<f64>,
    pub average_speed: Option<f64>,
    pub top_speed: Option<f64>,
}

impl URLParams {
    pub fn hash(self) -> String {
        let serialized =
            serde_json::to_string(&self).expect("Failed to serialize query for image key");
        utils::hash_string(&serialized)
    }
}
