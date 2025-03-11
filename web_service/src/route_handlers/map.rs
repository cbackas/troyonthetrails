use std::io::Cursor;

use axum::body::Body;
use axum::http::header;
use axum::response::IntoResponse;
use axum::{extract::Query, response::Response};

use ab_glyph::{FontRef, PxScale};
use geo_types::LineString;
use image::{load_from_memory, DynamicImage, ImageFormat, Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};
use polyline;
use serde::Deserialize;
use shared_lib::env_utils;
use staticmap::tools::Tool;
use staticmap::Bounds;
use staticmap::{tools::LineBuilder, StaticMapBuilder};
use tiny_skia::{PixmapMut, Transform};

#[derive(Deserialize)]
pub struct MapParams {
    pub polyline: String,
}

#[derive(Debug, Copy, Clone)]
pub enum TextAlignment {
    Center,
    Left,
    Right,
}

// Add to TextOptions
#[derive(Debug, Clone)]
pub struct TextOptions {
    pub color: Rgba<u8>,
    pub font_size: f32,
    pub alignment: TextAlignment, // New field
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            color: Rgba([255, 255, 255, 255]),
            font_size: 38.0,
            alignment: TextAlignment::Center, // Default to center
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

struct Darken {
    opacity: f32,
    extent: (f64, f64, f64, f64),
}

impl Tool for Darken {
    fn extent(&self, _: u8, _: f64) -> (f64, f64, f64, f64) {
        self.extent
    }

    fn draw(&self, _bounds: &Bounds, mut pixmap: PixmapMut) {
        let mut cover_pixmap = tiny_skia::Pixmap::new(1000, 1000).unwrap();
        let mut cover_pixmap = cover_pixmap.as_mut();
        cover_pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
        let cover_pixmap = cover_pixmap.as_ref();

        let paint = tiny_skia::PixmapPaint {
            opacity: self.opacity,
            blend_mode: tiny_skia::BlendMode::default(),
            quality: tiny_skia::FilterQuality::Nearest,
        };

        pixmap.draw_pixmap(0, 0, cover_pixmap, &paint, Transform::default(), None);
    }
}

enum TextElement {
    Text(String, TextOptions),
    TextWithSVG {
        text: String,
        options: TextOptions,
        svg_data: Vec<u8>,
    },
    Spacer,
}

struct MapImage {
    dynamic_img: DynamicImage,
    font: FontRef<'static>,
    elements: Vec<TextElement>,
}

impl MapImage {
    const IMAGE_WIDTH: u32 = 1000;
    const IMAGE_HEIGHT: u32 = 1000;

    pub fn new(polyline: &str) -> anyhow::Result<Self> {
        let font = {
            let font_data = include_bytes!("../../assets/AntonSC-Regular.ttf");
            FontRef::try_from_slice(font_data)?
        };

        let dynamic_img = {
            let line_string = Self::decode_polyline(polyline)?;
            Self::get_background_image(line_string)?
        };

        Ok(Self {
            dynamic_img,
            font,
            elements: Vec::new(),
        })
    }

    fn decode_polyline(polyline: &str) -> anyhow::Result<LineString> {
        let polyline = &polyline.replace("\\\\", "\\");
        let polyline = polyline::decode_polyline(polyline, 5)?;
        Ok(polyline)
    }

    fn get_background_image(line_string: LineString) -> anyhow::Result<DynamicImage> {
        let (lat_values, lng_values): (Vec<f64>, Vec<f64>) =
            line_string.coords().map(|coord| (coord.y, coord.x)).unzip();

        let url_template = {
            let key = env_utils::get_thunderforest_api_key()
                .expect("Failed to get Thunderforest API key");
            "https://c.tile.thunderforest.com/cycle/{z}/{x}/{y}.png?apikey=".to_string() + &key
        };

        let map_png = {
            let line = LineBuilder::default()
                .lat_coordinates(lat_values.clone())
                .lon_coordinates(lng_values.clone())
                .width(3.)
                .simplify(true)
                .color(staticmap::tools::Color::new(true, 255, 165, 0, 255))
                .build()?;

            let darken = Darken {
                opacity: 0.5,
                extent: line.extent(0, 0.0),
            };

            let mut map = StaticMapBuilder::default()
                .width(Self::IMAGE_WIDTH)
                .height(Self::IMAGE_HEIGHT)
                .padding((5, 0))
                .url_template(url_template)
                .build()?;

            map.add_tool(darken);
            map.add_tool(line);
            let map_image = map.encode_png()?;
            load_from_memory(&map_image)?
        };

        Ok(map_png)
    }

    pub fn add_text(&mut self, text: &str, options: impl Into<TextOptions>) -> &mut Self {
        self.elements
            .push(TextElement::Text(text.to_owned(), options.into()));
        self
    }

    pub fn add_text_with_svg(
        &mut self,
        text: &str,
        options: impl Into<TextOptions>,
        svg_data: &[u8],
    ) -> &mut Self {
        self.elements.push(TextElement::TextWithSVG {
            text: text.to_owned(),
            options: options.into(),
            svg_data: svg_data.to_vec(),
        });
        self
    }

    pub fn add_spacer(&mut self) -> &mut Self {
        self.elements.push(TextElement::Spacer);
        self
    }

    fn draw_all_text(&mut self) {
        const LINE_SPACING: i32 = 60;
        const HORIZONTAL_PADDING: i32 = 250;
        const IMAGE_WIDTH: i32 = MapImage::IMAGE_WIDTH as i32;
        const IMAGE_HEIGHT: i32 = MapImage::IMAGE_HEIGHT as i32;

        let total_elements = self
            .elements
            .iter()
            .filter(|e| matches!(e, TextElement::Text(_, _)))
            .count();
        let total_height = total_elements as i32 * LINE_SPACING;
        let mut current_y = (IMAGE_HEIGHT / 3) - (total_height / 2);

        let mut rgba_img = self.dynamic_img.to_rgba8();

        for element in &self.elements {
            match element {
                TextElement::Text(text, options) => {
                    let scale = PxScale::from(2.0 * options.font_size);

                    // text dimensions
                    let (text_width, _) = text_size(scale, &self.font, text);

                    let x = match options.alignment {
                        TextAlignment::Center => (IMAGE_WIDTH - text_width as i32) / 2,
                        TextAlignment::Left => HORIZONTAL_PADDING,
                        TextAlignment::Right => {
                            IMAGE_WIDTH - text_width as i32 - HORIZONTAL_PADDING
                        }
                    };

                    draw_text_mut(
                        &mut rgba_img,
                        options.color,
                        x,
                        current_y,
                        scale,
                        &self.font,
                        text,
                    );
                    current_y += LINE_SPACING;
                }

                TextElement::TextWithSVG {
                    text,
                    options,
                    svg_data,
                } => {
                    let svg_img = self
                        .render_svg(svg_data, options.font_size)
                        .expect("Failed to render SVG");

                    // combined width
                    let (text_width, _) = text_size(
                        PxScale {
                            x: options.font_size * 2.0,
                            y: options.font_size,
                        },
                        &self.font,
                        text,
                    );

                    let spacing = 30;

                    let total_width = svg_img.width() as i32 + spacing + text_width as i32;

                    let start_x = match options.alignment {
                        TextAlignment::Center => (IMAGE_WIDTH - total_width) / 2,
                        TextAlignment::Left => HORIZONTAL_PADDING,
                        TextAlignment::Right => IMAGE_WIDTH - total_width - HORIZONTAL_PADDING,
                    };

                    image::imageops::overlay(
                        &mut rgba_img,
                        &svg_img,
                        start_x as i64,
                        current_y as i64,
                    );

                    draw_text_mut(
                        &mut rgba_img,
                        options.color,
                        start_x + svg_img.width() as i32 + spacing,
                        current_y,
                        PxScale {
                            x: options.font_size * 2.0,
                            y: options.font_size,
                        },
                        &self.font,
                        text,
                    );

                    current_y += LINE_SPACING;
                }

                TextElement::Spacer => {
                    current_y += LINE_SPACING;
                }
            }
        }

        self.dynamic_img = DynamicImage::ImageRgba8(rgba_img);
    }

    fn render_svg(&self, svg_data: &[u8], target_height: f32) -> anyhow::Result<RgbaImage> {
        let opt = usvg::Options {
            resources_dir: None,
            font_family: "Arial".to_string(),
            font_size: target_height,
            ..usvg::Options::default()
        };

        let tree = usvg::Tree::from_data(svg_data, &opt)
            .map_err(|e| anyhow::anyhow!("SVG parse error: {}", e))?;

        let (_w, h) = tree.size().to_int_size().dimensions();
        let scale = if h > 0 { target_height / h as f32 } else { 2.0 };

        let pixmap_size = tree
            .size()
            .to_int_size()
            .scale_by(scale)
            .expect("Invalid SVG dimensions");

        let pixmap = {
            let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())
                .ok_or_else(|| anyhow::anyhow!("Invalid SVG dimensions"))?;

            resvg::render(
                &tree,
                tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );

            pixmap
        };

        let mut img_data = pixmap.data().to_vec();
        for pixel in img_data.chunks_exact_mut(4) {
            let a = pixel[3] as f32 / 255.0;
            if a > 0.0 {
                pixel[0] = (pixel[0] as f32 / a).min(255.0) as u8;
                pixel[1] = (pixel[1] as f32 / a).min(255.0) as u8;
                pixel[2] = (pixel[2] as f32 / a).min(255.0) as u8;
            }
        }

        RgbaImage::from_raw(pixmap.width(), pixmap.height(), img_data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))
    }

    pub fn encode_png(&mut self) -> anyhow::Result<Vec<u8>> {
        self.draw_all_text();
        let mut output_bytes = Vec::new();
        self.dynamic_img
            .write_to(&mut Cursor::new(&mut output_bytes), ImageFormat::Png)?;

        Ok(output_bytes)
    }
}

pub async fn handler(params: Query<MapParams>) -> impl axum::response::IntoResponse {
    let left_aligned = TextOptions {
        color: Rgba([255, 255, 255, 255]),
        font_size: 36.0,
        alignment: TextAlignment::Left,
    };

    let map_image = match MapImage::new(&params.polyline) {
        Ok(mut map_image) => match map_image
            .add_text("1 hour, 36 minute ride", DefaultColors::White)
            .add_spacer()
            .add_text_with_svg(
                "Rode 7.8 miles",
                left_aligned.clone(),
                include_bytes!("../../assets/measure-2-svgrepo-com.svg"),
            )
            .add_text_with_svg(
                "Climbed 445.9 feet",
                left_aligned.clone(),
                include_bytes!("../../assets/climb-svgrepo-com.svg"),
            )
            .add_text_with_svg(
                "Average speed of 2.8 mph",
                left_aligned.clone(),
                include_bytes!("../../assets/speedometer-svgrepo-com.svg"),
            )
            .add_text_with_svg(
                "Top speed of 9.0 mph",
                left_aligned.clone(),
                include_bytes!("../../assets/lightning-charge-svgrepo-com.svg"),
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
