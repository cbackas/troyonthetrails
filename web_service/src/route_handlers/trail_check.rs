use shared_lib::trail_structs::{TrailStatus, TrailSystem};

pub async fn handler() -> impl axum::response::IntoResponse {
    let trail_data_cache = trail_service::trail_data::get_data().await;
    super::html_template::HtmlTemplate(TrailCheckTemplate {
        trails: trail_data_cache.trail_data,
    })
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
