use shared_lib::trail_structs::{TrailStatus, TrailSystem};
use std::sync::Arc;

use axum::extract::State;
use tokio::sync::Mutex;
use tracing::log::error;

use crate::AppState;

pub async fn handler(
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl axum::response::IntoResponse {
    {
        let state = state.lock().await;
        if let Some(last_updated) = state.trail_data_last_updated {
            // if the trail data was updated less than 5 minutes ago, just use that
            if last_updated.elapsed().as_secs() < 300 {
                tracing::trace!("Using cached trail data");
                let template = TrailCheckTemplate {
                    trails: state.trail_data.clone(),
                };
                return super::html_template::HtmlTemplate(template);
            }
        }
    }

    let trail_data = match crate::trail_lib::get_trail_data().await {
        Ok(data) => sort_trail_data(data),
        Err(err) => {
            error!("Failed to get trail data: {}", err);
            return super::html_template::HtmlTemplate(TrailCheckTemplate { trails: vec![] });
        }
    };

    let template = TrailCheckTemplate {
        trails: trail_data.clone(),
    };

    // update the cached trail data
    {
        let mut state = state.lock().await;
        state.trail_data = trail_data;
        state.trail_data_last_updated = Some(tokio::time::Instant::now());
    }

    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "components/trail_check.html")]
struct TrailCheckTemplate {
    pub trails: Vec<TrailSystem>,
}

impl TrailCheckTemplate {
    pub fn format_time_ago(&self, datetime_str: &Option<String>) -> String {
        match datetime_str {
            Some(dt) => crate::utils::utc_to_time_ago_human_readable(dt),
            None => Default::default(),
        }
    }
}

fn sort_trail_data(trail_data: Vec<TrailSystem>) -> Vec<TrailSystem> {
    let static_lat = match std::env::var("HOME_LAT")
        .map_err(|e| e.to_string())
        .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
    {
        Ok(lat) => lat,
        Err(err) => {
            error!("Failed to parse HOME_LAT environment variable: {}", err);
            return trail_data;
        }
    };

    let static_lng = match std::env::var("HOME_LNG")
        .map_err(|e| e.to_string())
        .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
    {
        Ok(lng) => lng,
        Err(err) => {
            error!("Failed to parse HOME_LNG environment variable: {}", err);
            return trail_data;
        }
    };

    let mut sorted_data = trail_data;
    sorted_data.sort_by(|a, b| {
        let distance_a = ((a.lat - static_lat).powi(2) + (a.lng - static_lng).powi(2)).sqrt();
        let distance_b = ((b.lat - static_lat).powi(2) + (b.lng - static_lng).powi(2)).sqrt();
        distance_a
            .partial_cmp(&distance_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    sorted_data
}
