pub async fn handler() -> impl axum::response::IntoResponse {
    let trail_data_cache = trail_service::trail_data::get_data().await;
    let rides = strava_service::get_all_activities()
        .await
        .unwrap_or_default();

    let trail_stats =
        trail_service::ride_counts::calculate_stats(trail_data_cache.trail_data, rides);

    let mut response = String::new();
    for (id, stats) in trail_stats {
        if stats.rides == 0 {
            continue;
        }

        let total_moving_time = match stats.total_moving_time {
            0 => "never".to_string(),
            elapsed => {
                let hours = elapsed / 3600;
                let minutes = (elapsed % 3600) / 60;
                let seconds = elapsed % 60;

                let mut time_parts = Vec::new();
                if hours > 0 {
                    time_parts.push(format!("{}h", hours));
                }
                if minutes > 0 {
                    time_parts.push(format!("{}m", minutes));
                }
                if seconds > 0 {
                    time_parts.push(format!("{}s", seconds));
                }

                time_parts.join(" ")
            }
        };

        let template = TrailStatsTemplate {
            id,
            rides: format!("{} times", stats.rides),
            achievement_count: stats.achievement_count,
            total_moving_time,
        };
        response.push_str(&template.to_string());
    }

    axum::response::IntoResponse::into_response(response)
}

#[derive(askama::Template)]
#[template(path = "components/trail_stats.html")]
struct TrailStatsTemplate {
    pub id: u64,
    pub rides: String,
    pub achievement_count: i64,
    pub total_moving_time: String,
}
