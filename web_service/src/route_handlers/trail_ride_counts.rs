use shared_lib::trail_structs::TrailStatsDisplay;

pub async fn handler() -> impl axum::response::IntoResponse {
    let trail_data_cache = trail_service::trail_data::get_data().await;
    let rides = strava_service::get_all_activities()
        .await
        .unwrap_or_default();

    let trail_stats =
        trail_service::ride_counts::calculate_stats(trail_data_cache.trail_data, rides);

    let response: String = trail_stats
        .into_iter()
        .filter(|(_, stats)| stats.rides != 0)
        .map(|(_, stats)| {
            let template = TrailStatsTemplate {
                stats: stats.into(),
                swap_oob: true,
            };
            template.to_string()
        })
        .collect();

    axum::response::IntoResponse::into_response(response)
}

#[derive(askama::Template)]
#[template(path = "components/trail_stats.html")]
struct TrailStatsTemplate {
    pub stats: TrailStatsDisplay,
    pub swap_oob: bool,
}
