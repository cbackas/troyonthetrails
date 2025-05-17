use std::io::Cursor;

use ab_glyph::{FontRef, PxScale};
use geo_types::LineString;
use image::{load_from_memory, DynamicImage, ImageFormat, Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};
use shared_lib::env_utils;
use staticmap::tools::Tool;
use staticmap::Bounds;
use staticmap::{tools::LineBuilder, StaticMapBuilder};
use tiny_skia::{PixmapMut, Transform};

const IMAGE_WIDTH: u32 = 900;
const IMAGE_HEIGHT: u32 = 900;

#[derive(Debug, Copy, Clone)]
pub enum DefaultColor {
    White,
    Orange,
    Blue,
    Red,
    Green,
}

impl From<DefaultColor> for Rgba<u8> {
    fn from(val: DefaultColor) -> Self {
        match val {
            DefaultColor::White => Rgba([255, 255, 255, 255]),
            DefaultColor::Orange => Rgba([255, 165, 0, 255]),
            DefaultColor::Blue => Rgba([0, 0, 255, 255]),
            DefaultColor::Red => Rgba([255, 0, 0, 255]),
            DefaultColor::Green => Rgba([0, 128, 0, 255]),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum TextAlignment {
    Center,
    Left,
}

// Add to TextOptions
#[derive(Debug, Clone)]
pub struct TextOptions {
    pub color: DefaultColor,
    pub font_size: f32,
    pub alignment: TextAlignment,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            color: DefaultColor::White,
            font_size: 38.0,
            alignment: TextAlignment::Center,
        }
    }
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
        let mut cover_pixmap = tiny_skia::Pixmap::new(IMAGE_WIDTH, IMAGE_HEIGHT).unwrap();
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

pub struct MapImage {
    dynamic_img: DynamicImage,
    font: FontRef<'static>,
    elements: Vec<TextElement>,
}

impl MapImage {
    pub fn new(polyline: &str) -> anyhow::Result<Self> {
        let font = {
            let font_data = include_bytes!("../assets/PTSans-Bold.ttf");
            FontRef::try_from_slice(font_data)?
        };

        let dynamic_img = {
            let line_string = polyline::decode_polyline(polyline, 5)?;
            Self::get_background_image(line_string)?
        };

        Ok(Self {
            dynamic_img,
            font,
            elements: Vec::new(),
        })
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
                opacity: 0.65,
                extent: line.extent(0, 0.0),
            };

            let mut map = StaticMapBuilder::default()
                .width(IMAGE_WIDTH)
                .height(IMAGE_HEIGHT)
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
        const LINE_SPACING: i32 = 80;
        const IIMAGE_WIDTH: i32 = IMAGE_WIDTH as i32;
        const IIMAGE_HEIGHT: i32 = IMAGE_HEIGHT as i32;
        const HORIZONTAL_PADDING: i32 = IIMAGE_WIDTH / 8;

        let total_elements = self.elements.len();
        let total_height = total_elements as i32 * LINE_SPACING;
        let mut current_y = (IIMAGE_HEIGHT / 2) - (total_height / 2);

        let mut rgba_img = self.dynamic_img.to_rgba8();

        for element in &self.elements {
            match element {
                TextElement::Text(text, options) => {
                    let scale = PxScale {
                        x: options.font_size * 2.0,
                        y: options.font_size * 2.0,
                    };

                    // text dimensions
                    let (text_width, _) = text_size(scale, &self.font, text);

                    let x = match options.alignment {
                        TextAlignment::Center => (IIMAGE_WIDTH - text_width as i32) / 2,
                        TextAlignment::Left => HORIZONTAL_PADDING,
                    };

                    draw_text_mut(
                        &mut rgba_img,
                        options.color.into(),
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
                        .render_svg(svg_data, options.font_size * 2.0)
                        .expect("Failed to render SVG");

                    let scale = PxScale {
                        x: options.font_size * 2.0,
                        y: options.font_size * 2.0,
                    };

                    // combined width
                    let (text_width, _) = text_size(scale, &self.font, text);

                    let spacing = 15;

                    let total_width = svg_img.width() as i32 + spacing + text_width as i32;

                    let start_x = match options.alignment {
                        TextAlignment::Center => (IIMAGE_WIDTH - total_width) / 2,
                        TextAlignment::Left => HORIZONTAL_PADDING,
                    };

                    image::imageops::overlay(
                        &mut rgba_img,
                        &svg_img,
                        start_x as i64,
                        current_y as i64,
                    );

                    draw_text_mut(
                        &mut rgba_img,
                        options.color.into(),
                        start_x + svg_img.width() as i32 + spacing,
                        current_y,
                        scale,
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
