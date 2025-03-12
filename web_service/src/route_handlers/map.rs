use axum::body::Body;
use axum::http::header;
use axum::response::IntoResponse;
use axum::{extract::Query, response::Response};

use crate::map_image::{DefaultColor, MapImage, TextAlignment, TextOptions};

pub async fn handler(
    params: Query<shared_lib::structs::URLParams>,
) -> impl axum::response::IntoResponse {
    let polyline = match &params.polyline {
        Some(polyline) => polyline,
        None => {
            tracing::error!("Missing polyline parameter");
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };

    const TITLE_ROW_HEIGHT: f32 = 65.0;
    const DATA_ROW_HEIGHT: f32 = 36.0;

    let mut map_image = match MapImage::new(polyline) {
        Ok(map_image) => map_image,
        Err(err) => {
            tracing::error!("Failed to create map image: {:?}", err);
            return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Some(title) = &params.title {
        map_image
            .add_text(
                title,
                TextOptions {
                    color: DefaultColor::White,
                    font_size: TITLE_ROW_HEIGHT,
                    alignment: TextAlignment::Center,
                },
            )
            .add_spacer()
            .add_spacer();
    }

    if let Some(duration) = params.duration {
        let duration = shared_lib::utils::minutes_to_human_readable(duration);
        map_image
            .add_text(
                format!("{} ride", duration).as_str(),
                TextOptions {
                    color: DefaultColor::White,
                    font_size: DATA_ROW_HEIGHT,
                    alignment: TextAlignment::Center,
                },
            )
            .add_spacer();
    }

    if let Some(distance) = params.distance {
        let distance = shared_lib::utils::meters_to_miles(distance, false);
        map_image.add_text_with_svg(
            format!("Rode {} miles", distance).as_str(),
            TextOptions {
                color: DefaultColor::White,
                font_size: DATA_ROW_HEIGHT,
                alignment: TextAlignment::Left,
            },
            include_bytes!("../../assets/measure-2-svgrepo-com.svg"),
        );
    }

    if let Some(elevation_gain) = params.elevation_gain {
        let elevation_gain = shared_lib::utils::meters_to_feet(elevation_gain, false);
        map_image.add_text_with_svg(
            format!("Climbed {} feet", elevation_gain).as_str(),
            TextOptions {
                color: DefaultColor::White,
                font_size: DATA_ROW_HEIGHT,
                alignment: TextAlignment::Left,
            },
            include_bytes!("../../assets/climb-svgrepo-com.svg"),
        );
    }

    if let Some(average_speed) = params.average_speed {
        map_image.add_text_with_svg(
            format!("Average speed of {:.1} mph", average_speed).as_str(),
            TextOptions {
                color: DefaultColor::White,
                font_size: DATA_ROW_HEIGHT,
                alignment: TextAlignment::Left,
            },
            include_bytes!("../../assets/speedometer-svgrepo-com.svg"),
        );
    }

    if let Some(top_speed) = params.top_speed {
        map_image.add_text_with_svg(
            format!("Top speed of {:.1} mph", top_speed).as_str(),
            TextOptions {
                color: DefaultColor::White,
                font_size: DATA_ROW_HEIGHT,
                alignment: TextAlignment::Left,
            },
            include_bytes!("../../assets/lightning-charge-svgrepo-com.svg"),
        );
    }

    let map_image = match map_image.encode_png() {
        Ok(map_image) => map_image,
        Err(err) => {
            tracing::error!("Failed to encode map image: {:?}", err);
            return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(
            header::CONTENT_DISPOSITION,
            &format!("filename=\"{:?}.png\"", "trail_map"),
        )
        .body(Body::from(map_image))
        .expect("Failed to build response")
        .into_response()
}
