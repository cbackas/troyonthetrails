use std::io::Cursor;

use axum::body::Body;
use axum::http::header;
use axum::response::IntoResponse;
use axum::{extract::Query, response::Response};

use ab_glyph::{FontRef, PxScale};
use geo_types::LineString;
use image::{load_from_memory, DynamicImage, ImageFormat};
use imageproc::drawing::draw_text_mut;
use polyline;
use serde::Deserialize;
use shared_lib::env_utils;
use staticmap::{tools::LineBuilder, StaticMapBuilder};

#[derive(Deserialize)]
pub struct MapParams {
    pub polyline: String,
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
                .color(staticmap::tools::Color::new(true, 255, 0, 0, 255))
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

    pub fn add_text(&mut self, text: &str, x: i32, y: i32, font_size: f32) -> &mut Self {
        let mut rgba_img = {
            let dyanmic_img = &self.dynamic_img;
            dyanmic_img.to_rgba8()
        };

        let scale = PxScale {
            // horizontal scaling
            x: font_size * 2.0,
            // Vertical scaling
            y: font_size,
        };

        // define text color (Rgba<u8>)
        let text_color = image::Rgba([255u8, 255u8, 255u8, 255u8]);

        draw_text_mut(&mut rgba_img, text_color, x, y, scale, &self.font, text);
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
    let mut map_image = match MapImage::new(&params.polyline) {
        Ok(map_image) => map_image,
        Err(err) => {
            tracing::error!("Failed to create map image: {:?}", err);
            return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    map_image
        .add_text("Hello, World1!", 1600 / 2, 1600 / 2, 60.0)
        .add_text("Hello, World2!", 1600 / 2, 1600 / 2 + 100, 60.0)
        .add_text("Hello, World3!", 1600 / 2, 1600 / 2 + 200, 60.0);
    let map_image = map_image.encode_png().expect("Failed to build map image");

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
