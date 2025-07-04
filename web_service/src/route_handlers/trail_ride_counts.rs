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

        let rides = match stats.rides {
            1 => "1 time".to_string(),
            _ => format!("{} times", stats.rides),
        };

        let total_moving_time = match stats.total_moving_time {
            0 => "never".to_string(),
            elapsed => {
                let hours = (elapsed as f64 / 3600.0 * 2.0).round() / 2.0;
                if hours.fract() == 0.0 {
                    format!("{:.0}h", hours)
                } else {
                    format!("{:.1}h", hours)
                }
            }
        };

        let template = TrailStatsTemplate {
            id,
            rides,
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
