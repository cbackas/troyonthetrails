use std::sync::Arc;

use axum::extract::State;
use tokio::{sync::Mutex, time::Instant};

use crate::AppState;

pub async fn handler(
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl axum::response::IntoResponse {
    let mut state = state.lock().await;

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

    let template = HomeTemplate { last_updated };
    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    last_updated: String,
}
