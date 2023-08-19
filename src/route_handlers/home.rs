use std::sync::Arc;

use axum::extract::State;
use tokio::sync::Mutex;

use crate::AppState;

pub async fn handler(
    State(_state): State<Arc<Mutex<AppState>>>,
) -> impl axum::response::IntoResponse {
    let template = HomeTemplate {};
    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {}
