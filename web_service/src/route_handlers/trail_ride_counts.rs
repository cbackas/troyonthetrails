pub async fn handler() -> impl axum::response::IntoResponse {
    let trail_data_cache = trail_service::trail_data::get_data().await;
    let rides = strava_service::get_all_activities()
        .await
        .unwrap_or_default();

    let counts = trail_service::ride_counts::calculate_counts(trail_data_cache.trail_data, rides);

    let mut response = String::new();
    for (id, count) in counts {
        response.push_str(&format!(
            r#"<div id="count-{}" hx-swap-oob="true">{}</div>"#,
            id, count
        ));
    }

    axum::response::IntoResponse::into_response(response)
}
