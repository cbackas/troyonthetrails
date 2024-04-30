use crate::db_service::get_troy_status;

pub async fn handler() -> impl axum::response::IntoResponse {
    let troy_status = get_troy_status().await;
    let template = TrailCheckTemplate {
        is_troy_on_the_trails: troy_status.is_on_trail,
    };
    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "components/troy_check.html")]
struct TrailCheckTemplate {
    is_troy_on_the_trails: bool,
}
