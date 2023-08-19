use std::sync::Arc;

use axum::extract::State;
use tokio::sync::Mutex;

use crate::AppState;

pub async fn handler(
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl axum::response::IntoResponse {
    let state = state.lock().await;
    let template = TrailCheckTemplate {
        is_troy_on_the_trails: state.is_troy_on_the_trails,
    };
    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "components/troy_check.html")]
struct TrailCheckTemplate {
    is_troy_on_the_trails: bool,
}
