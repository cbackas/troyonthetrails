use std::sync::Arc;

use axum::extract::State;
use tokio::{sync::Mutex, time::Instant};

use crate::{strava_api_service::API_SERVICE, AppState};

pub async fn handler(app_state: State<Arc<Mutex<AppState>>>) -> impl axum::response::IntoResponse {
    let mut state = app_state.lock().await;

    let last_updated = match state.troy_status_last_updated {
        None => "never".to_string(),
        Some(updated) => {
            let elapsed = updated.elapsed();

            // if its been more than 4 hours then troy problably isn't on the trails
            if elapsed.as_secs() > 14400 {
                state.is_troy_on_the_trails = false;
                state.troy_status_last_updated = Some(Instant::now());
            }

            let elapsed = humantime::format_duration(elapsed).to_string();
            let elapsed: Vec<&str> = elapsed.split_whitespace().collect();
            let elapsed = elapsed[..elapsed.len() - 3].join(" ");
            format!("{} ago", elapsed)
        }
    };

    let api_service = API_SERVICE.lock().await;
    let has_strava_token = api_service.token_data.is_some();

    let template = HomeTemplate {
        last_updated,
        has_strava_token,
    };
    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    last_updated: String,
    has_strava_token: bool,
}
