use crate::utils;

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct URLParams {
    pub title: Option<String>,
    pub polyline: Option<String>,
    pub duration: Option<String>,
    pub distance: Option<String>,
    pub elevation_gain: Option<String>,
    pub average_speed: Option<String>,
    pub top_speed: Option<String>,
    pub as_image: Option<bool>,
}

impl URLParams {
    pub fn hash(mut self) -> String {
        // let mut query = self.clone();

        // we dont include the as_image field in the hash cuz its not real data
        self.as_image = None;

        let serialized =
            serde_json::to_string(&self).expect("Failed to serialize query for image key");
        utils::hash_string(&serialized)
    }
}
