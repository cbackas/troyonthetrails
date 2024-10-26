use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Query, http::header::CONTENT_TYPE, response::Response};

use crate::browser;
use crate::shared_lib::structs::URLParams;

#[derive(askama::Template)]
#[template(path = "index.html")]
struct HTMLTemplate {
    ride_title: Option<String>,
    duration: Option<String>,
    distance: Option<String>,
    elevation_gain: Option<String>,
    average_speed: Option<String>,
    top_speed: Option<String>,
}

#[derive(askama::Template)]
#[template(path = "assets/index.js", escape = "none")]
struct JSTemplate {
    thunderforest_api_key: String,
}

pub async fn route_handler(params: Query<URLParams>) -> impl axum::response::IntoResponse {
    match params.as_image {
        Some(true) => image_handler(params).await,
        _ => html_handler(params).await,
    }
}

pub async fn html_handler(params: Query<URLParams>) -> Response {
    let duration: Option<String> = params
        .duration
        .as_ref()
        .map(|duration| minutes_to_human_readable(duration.parse().unwrap_or_default()));

    let distance: Option<String> = params.distance.as_ref().map(|distance| {
        let distance = distance.parse().unwrap_or_default();
        let distance = crate::shared_lib::utils::meters_to_miles(distance, false);
        distance.to_string()
    });

    let elevation_gain: Option<String> = params.elevation_gain.as_ref().map(|elevation_gain| {
        let elevation_gain = elevation_gain.parse().unwrap_or_default();
        let elevation_gain = crate::shared_lib::utils::meters_to_feet(elevation_gain, false);
        elevation_gain.to_string()
    });

    let average_speed: Option<String> = params.average_speed.as_ref().map(|average_speed| {
        let average_speed: f64 = average_speed.parse().unwrap_or_default();
        format!("{:.1}", average_speed)
    });

    let top_speed: Option<String> = params.top_speed.as_ref().map(|top_speed| {
        let top_speed: f64 = top_speed.parse().unwrap_or_default();
        format!("{:.1}", top_speed)
    });

    let template: HTMLTemplate = HTMLTemplate {
        ride_title: params.title.clone(),
        duration,
        distance,
        elevation_gain,
        average_speed,
        top_speed,
    };
    super::html_template::HtmlTemplate(template).into_response()
}

pub async fn image_handler(Query(query): Query<URLParams>) -> Response {
    let query_key = query.clone().hash();

    let query = {
        let mut query = query;
        query.as_image = Some(false);
        serde_urlencoded::to_string(query).expect("Failed to serialize query for image")
    };
    let data =
        match browser::get_screenshot(format!("http://localhost:7070/?{}", query).as_str()).await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Failed to get tab: {:?}", e);
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
                    .into_response();
            }
        };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(
            header::CONTENT_DISPOSITION,
            &format!("filename=\"{:?}.png\"", query_key),
        )
        .body(Body::from(data))
        .unwrap()
        .into_response()
}

pub async fn js_handler() -> impl axum::response::IntoResponse {
    let contents = JSTemplate {
        thunderforest_api_key: crate::env_utils::get_thunderforest_api_key().unwrap_or_default(),
    };
    Response::builder()
        .header(CONTENT_TYPE, "application/javascript")
        .body(contents.to_string())
        .unwrap()
}

pub async fn css_handler() -> impl axum::response::IntoResponse {
    let contents = include_str!("../templates/assets/index.css");
    Response::builder()
        .header(CONTENT_TYPE, "text/css")
        .body(contents.to_string())
        .unwrap()
}

fn minutes_to_human_readable(seconds: i64) -> String {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let mins = minutes % 60;

    match hours {
        0 => format!("{} minute", mins),
        _ => format!("{} hour, {} minute", hours, mins),
    }
}
