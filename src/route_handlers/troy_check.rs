use crate::db_service::DB_SERVICE;

pub async fn handler() -> impl axum::response::IntoResponse {
    let db_service = DB_SERVICE.lock().await;
    let is_troy_on_the_trails = db_service.get_troy_status().is_on_trail;

    let template = TrailCheckTemplate {
        is_troy_on_the_trails,
    };
    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "components/troy_check.html")]
struct TrailCheckTemplate {
    is_troy_on_the_trails: bool,
}
