use shared_lib::trail_structs::{TrailStatus, TrailSystem};

pub async fn handler() -> impl axum::response::IntoResponse {
    let trail_data_cache = trail_service::trail_data::get_data().await;
    let trails = match strava_service::get_cached_activities().await {
        None => trail_data_cache.trail_data,
        Some(rides) => {
            let trail_stats = trail_service::ride_counts::calculate_stats(
                trail_data_cache.trail_data.clone(),
                rides,
            );

            trail_data_cache
                .trail_data
                .into_iter()
                .map(|mut trail| {
                    if let Some(stats) = trail_stats.get(&trail.id) {
                        trail.stats = Some((*stats).into());
                    }
                    trail
                })
                .filter(|trail| trail.status != TrailStatus::Unknown)
                .collect::<Vec<TrailSystem>>()
        }
    };

    super::html_template::HtmlTemplate(TrailCheckTemplate { trails })
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
