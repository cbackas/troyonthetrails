use axum::response::IntoResponse;
use tracing::error;

use crate::{
    data_utils::{format_thousands, meters_to_feet, meters_to_miles},
    strava_api_service::API_SERVICE,
};

pub async fn handler() -> impl IntoResponse {
    let mut api_service = API_SERVICE.lock().await;

    let strava_data = match api_service.get_athlete_stats().await {
        Ok(data) => data,
        Err(err) => {
            error!("Failed to get strava data: {}", err);
            return axum::http::status::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let total_rides = strava_data.all_ride_totals.count as f64;
    let total_distance = meters_to_miles(strava_data.all_ride_totals.distance, true);
    let total_elevation_gain = meters_to_feet(strava_data.all_ride_totals.elevation_gain, true);
    let longest_ride = meters_to_miles(strava_data.biggest_ride_distance, true);

    super::html_template::HtmlTemplate(StravaDataTemplate {
        total_rides: format_thousands(total_rides),
        total_distance: format_thousands(total_distance),
        total_elevation_gain: format_thousands(total_elevation_gain),
        longest_ride: format_thousands(longest_ride),
    })
    .into_response()
}

#[derive(askama::Template)]
#[template(path = "components/strava_data.html")]
struct StravaDataTemplate {
    total_rides: String,
    total_distance: String,
    total_elevation_gain: String,
    longest_ride: String,
}
