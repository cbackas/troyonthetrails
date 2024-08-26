use axum::{
    http::header::CONTENT_TYPE,
    response::{Html, Response},
};

pub async fn html_handler() -> impl axum::response::IntoResponse {
    let contents = include_str!("../dist/index.html");
    Html(contents)
}

pub async fn js_handler() -> impl axum::response::IntoResponse {
    let contents = include_str!("../dist/assets/index.js");
    Response::builder()
        .header(CONTENT_TYPE, "application/javascript")
        .body(contents.to_string())
        .unwrap()
}

pub async fn css_handler() -> impl axum::response::IntoResponse {
    let contents = include_str!("../dist/assets/index.css");
    Response::builder()
        .header(CONTENT_TYPE, "text/css")
        .body(contents.to_string())
        .unwrap()
}
