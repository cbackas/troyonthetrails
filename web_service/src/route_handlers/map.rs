use std::io::Cursor;

use axum::body::Body;
use axum::http::header;
use axum::response::IntoResponse;
use axum::{extract::Query, response::Response};

use ab_glyph::{FontRef, PxScale};
use geo_types::LineString;
use image::{load_from_memory, DynamicImage, ImageFormat, Rgba};
use imageproc::drawing::draw_text_mut;
use polyline;
use serde::Deserialize;
use shared_lib::env_utils;
use staticmap::{tools::LineBuilder, StaticMapBuilder};

#[derive(Deserialize)]
pub struct MapParams {
    pub polyline: String,
}

struct TextOptions {
    font_size: f32,
    color: Rgba<u8>,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            font_size: 60.0,
            color: Rgba([255u8, 255u8, 255u8, 255u8]),
        }
    }
}

impl From<DefaultColors> for TextOptions {
    fn from(color: DefaultColors) -> Self {
        match color {
            DefaultColors::White => Self::default(),
            DefaultColors::Orange => Self {
                color: Rgba([255u8, 165u8, 0u8, 255u8]),
                ..Self::default()
            },
            DefaultColors::Blue => Self {
                color: Rgba([0u8, 0u8, 255u8, 255u8]),
                ..Self::default()
            },
            DefaultColors::Red => Self {
                color: Rgba([255u8, 0u8, 0u8, 255u8]),
                ..Self::default()
            },
            DefaultColors::Green => Self {
                color: Rgba([0u8, 128u8, 0u8, 255u8]),
                ..Self::default()
            },
        }
    }
}

enum DefaultColors {
    White,
    Orange,
    Blue,
    Red,
    Green,
}

struct MapImage {
    dynamic_img: DynamicImage,
    font: FontRef<'static>,
}

impl MapImage {
    pub fn new(polyline: &str) -> anyhow::Result<Self> {
        let font = {
            let font_data = include_bytes!("../../assets/AntonSC-Regular.ttf");
            FontRef::try_from_slice(font_data)?
        };

        let dynamic_img = {
            let line_string = Self::decode_polyline(polyline)?;
            let background_image = Self::get_background_image(line_string)?;
            load_from_memory(&background_image)?
        };

        Ok(Self { dynamic_img, font })
    }

    fn decode_polyline(polyline: &str) -> anyhow::Result<LineString> {
        let polyline = &polyline.replace("\\\\", "\\");
        let polyline = polyline::decode_polyline(polyline, 5)?;
        Ok(polyline)
    }

    fn get_background_image(line_string: LineString) -> anyhow::Result<Vec<u8>> {
        let (lat_values, lng_values): (Vec<f64>, Vec<f64>) =
            line_string.coords().map(|coord| (coord.y, coord.x)).unzip();

        let url_template = {
            let key = env_utils::get_thunderforest_api_key()
                .expect("Failed to get Thunderforest API key");
            "https://c.tile.thunderforest.com/cycle/{z}/{x}/{y}.png?apikey=".to_string() + &key
        };

        let map_png = {
            let line = LineBuilder::default()
                .lat_coordinates(lat_values)
                .lon_coordinates(lng_values)
                .width(3.)
                .simplify(true)
                .color(staticmap::tools::Color::new(true, 255, 165, 0, 255))
                .build()?;

            let mut map = StaticMapBuilder::default()
                .width(1600)
                .height(1600)
                .padding((10, 0))
                .url_template(url_template)
                .build()?;

            map.add_tool(line);
            map.encode_png()?
        };

        Ok(map_png)
    }

    pub fn add_text(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        options: impl Into<TextOptions>,
    ) -> &mut Self {
        let options: TextOptions = options.into();

        let mut rgba_img = {
            let dyanmic_img = &self.dynamic_img;
            dyanmic_img.to_rgba8()
        };

        let scale = PxScale {
            // horizontal scaling
            x: options.font_size * 2.0,
            // Vertical scaling
            y: options.font_size,
        };

        draw_text_mut(&mut rgba_img, options.color, x, y, scale, &self.font, text);
        self.dynamic_img = DynamicImage::ImageRgba8(rgba_img);

        self
    }

    pub fn encode_png(&mut self) -> anyhow::Result<Vec<u8>> {
        let mut output_bytes = Vec::new();
        self.dynamic_img
            .write_to(&mut Cursor::new(&mut output_bytes), ImageFormat::Png)?;

        Ok(output_bytes)
    }
}

pub async fn handler(params: Query<MapParams>) -> impl axum::response::IntoResponse {
    let map_image = match MapImage::new(&params.polyline) {
        Ok(mut map_image) => match map_image
            .add_text("Hello, World1!", 1600 / 2 + 0, 1600 / 2, DefaultColors::Red)
            .add_text(
                "Hello, World2!",
                1600 / 2,
                1600 / 2 + 100,
                DefaultColors::Orange,
            )
            .add_text(
                "Hello, World3!",
                1600 / 2,
                1600 / 2 + 200,
                DefaultColors::White,
            )
            .encode_png()
        {
            Ok(map_image) => map_image,
            Err(err) => {
                tracing::error!("Failed to encode map image: {:?}", err);
                return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        },
        Err(err) => {
            tracing::error!("Failed to create map image: {:?}", err);
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
