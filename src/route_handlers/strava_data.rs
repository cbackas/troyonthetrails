use axum::response::IntoResponse;
use tracing::error;

use crate::strava_api_service::API_SERVICE;

pub async fn handler() -> impl IntoResponse {
    let mut api_service = API_SERVICE.lock().await;

    let strava_data = match api_service.get_athelete_stats().await {
        Ok(data) => data,
        Err(err) => {
            error!("Failed to get strava data: {}", err);
            return axum::http::status::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    super::html_template::HtmlTemplate(StravaDataTemplate {
        total_rides: format_thousands(strava_data.all_ride_totals.count as f64),
        total_distance: format_thousands((strava_data.all_ride_totals.distance / 1609.0).round()),
        total_elevation_gain: format_thousands(
            (strava_data.all_ride_totals.elevation_gain * 3.281).round(),
        ),
        longest_ride: format_thousands((strava_data.biggest_ride_distance / 1609.0).round()),
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

fn format_thousands(num: f64) -> String {
    let binding = num.to_string();
    let parts: Vec<&str> = binding.split('.').collect();
    let mut chars: Vec<char> = parts[0].chars().collect();
    let mut index = chars.len() as isize - 3;
    while index > 0 {
        chars.insert(index as usize, ',');
        index -= 3;
    }
    let integer_part: String = chars.into_iter().collect();
    if parts.len() > 1 {
        format!("{}.{}", integer_part, parts[1])
    } else {
        integer_part
    }
}
