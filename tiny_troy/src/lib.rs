use wasm_bindgen::prelude::*;
use web_sys::{window, CanvasRenderingContext2d, HtmlCanvasElement};

const CANVAS_WIDTH: f64 = 800.0;
const CANVAS_HEIGHT: f64 = 600.0;
const PLAYER_X_RATIO: f64 = 0.25; // 25% from left
const GRAVITY: f64 = 0.4;
const THRUST_FORCE: f64 = 2.5;  // Positive = downward acceleration (much stronger)
const FRICTION: f64 = 0.96;

#[wasm_bindgen]
pub struct Game {
    ctx: CanvasRenderingContext2d,
    canvas_width: f64,
    canvas_height: f64,

    // Player physics
    player_y: f64,
    player_vy: f64, // vertical velocity
    player_vx: f64, // horizontal velocity (momentum from hills)

    // World state
    world_offset: f64, // How far we've scrolled (X)
    camera_y: f64,     // Camera vertical position (follows player)
    time: f64,
    score: u32,
    game_over: bool,

    // Input
    space_pressed: bool,

    // Terrain
    hill_segments: Vec<HillSegment>,
}

#[derive(Clone)]
struct HillSegment {
    x: f64,
    y: f64,
    next_y: f64,
    slope: f64,
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Game, JsValue> {
        let window = window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        let canvas = document
            .get_element_by_id("gameCanvas")
            .ok_or("No canvas element")?
            .dyn_into::<HtmlCanvasElement>()?;

        let ctx = canvas
            .get_context("2d")?
            .ok_or("Failed to get 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()?;

        // Get canvas container size (or use defaults)
        let parent = canvas.parent_element().ok_or("No parent element")?;
        let parent_width = parent.client_width() as f64;
        let parent_height = parent.client_height() as f64;

        let canvas_width = if parent_width > 0.0 {
            parent_width
        } else {
            CANVAS_WIDTH
        };
        let canvas_height = if parent_height > 0.0 {
            parent_height
        } else {
            CANVAS_HEIGHT
        };

        // Set canvas size
        canvas.set_width(canvas_width as u32);
        canvas.set_height(canvas_height as u32);

        let mut game = Game {
            ctx,
            canvas_width,
            canvas_height,
            player_y: 100.0,
            player_vy: 0.0,
            player_vx: 4.0,
            world_offset: 0.0,
            camera_y: 0.0,
            time: 0.0,
            score: 0,
            game_over: false,
            space_pressed: false,
            hill_segments: Vec::new(),
        };

        game.generate_terrain();
        Ok(game)
    }

    fn generate_terrain(&mut self) {
        self.hill_segments.clear();

        let mut x = 0.0;
        let center_y = self.canvas_height / 2.0;
        let segment_width = 5.0;

        // Generate smooth sine-wave terrain like Tiny Wings
        for i in 0..20000 {
            // Progressive difficulty
            let progression = (i as f64) / 2000.0;

            // Multiple sine waves at different frequencies create natural hills
            let freq1 = i as f64 * 0.02;
            let freq2 = i as f64 * 0.05;
            let freq3 = i as f64 * 0.01;

            // Combine waves with different amplitudes
            let wave = freq1.sin() * 0.5 + freq2.sin() * 0.3 + freq3.sin() * 0.2;

            // Varying depth creates interesting terrain
            let base_depth = 80.0 + (progression * 60.0).min(120.0);
            let y = center_y + wave * base_depth;

            // Calculate next point
            let next_freq1 = (i as f64 + 1.0) * 0.02;
            let next_freq2 = (i as f64 + 1.0) * 0.05;
            let next_freq3 = (i as f64 + 1.0) * 0.01;
            let next_wave = next_freq1.sin() * 0.5 + next_freq2.sin() * 0.3 + next_freq3.sin() * 0.2;
            let next_y = center_y + next_wave * base_depth;

            let slope = (next_y - y) / segment_width;

            self.hill_segments.push(HillSegment {
                x,
                y,
                next_y,
                slope,
            });

            x += segment_width;
        }
    }

    pub fn set_space_pressed(&mut self, pressed: bool) {
        self.space_pressed = pressed;
    }

    pub fn update(&mut self) {
        if self.game_over {
            return;
        }

        self.time += 1.0;

        // Apply gravity
        self.player_vy += GRAVITY;

        // Apply thrust when space is pressed
        if self.space_pressed {
            self.player_vy += THRUST_FORCE;
        }

        // Apply momentum decay
        self.player_vy *= FRICTION;

        // Update player position
        self.player_y += self.player_vy;

        // Scroll world based on player velocity
        self.world_offset += self.player_vx;

        // Update camera to follow player (keep player at ~40% from top of screen)
        let target_screen_y = self.canvas_height * 0.4;
        self.camera_y = self.player_y - target_screen_y;

        // Get terrain height at player position
        let player_x = self.canvas_width * PLAYER_X_RATIO;
        let terrain_y = self.get_terrain_height_at(player_x + self.world_offset);

        // Collision detection (player radius is 6.0)
        let player_radius = 6.0;
        if self.player_y + player_radius >= terrain_y {
            self.player_y = terrain_y - player_radius;

            // Get slope at this position
            let slope = self.get_slope_at(player_x + self.world_offset);

            // Physics: downhill slopes (positive slope) add speed, uphill (negative) removes speed
            // Slope is (next_y - y) / segment_width
            if slope > 0.0 {
                // Going downhill - accelerate
                self.player_vx += slope * 0.15;
            } else {
                // Going uphill - decelerate (less aggressive)
                self.player_vx += slope * 0.1;
            }

            // Clamp speed to reasonable range
            self.player_vx = self.player_vx.max(0.5).min(15.0);

            // Small bounce when landing
            if self.player_vy > 0.5 {
                self.player_vy = -self.player_vy * 0.2;
            } else {
                self.player_vy = 0.0;
            }

            // Award points for staying on hills
            self.score += 1;
        }

        // Game over if falls off screen (check screen coordinates relative to camera)
        let player_screen_y = self.player_y - self.camera_y;
        if player_screen_y > self.canvas_height + 100.0 || player_screen_y < -100.0 {
            self.game_over = true;
        }

        // Game continues indefinitely with pre-generated terrain
    }

    pub fn render(&self) {
        // Clear canvas
        self.ctx.set_fill_style_str("#f0f0f0");
        self.ctx
            .fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

        // Draw terrain
        self.draw_terrain();

        // Draw player (simple circle for minimalist style)
        let player_x = self.canvas_width * PLAYER_X_RATIO;
        let player_screen_y = self.player_y - self.camera_y;
        self.ctx.set_fill_style_str("#000");
        self.ctx.begin_path();
        self.ctx
            .arc(
                player_x,
                player_screen_y,
                6.0,
                0.0,
                std::f64::consts::PI * 2.0,
            )
            .unwrap();
        self.ctx.fill();

        // Debug: draw collision point
        let terrain_y_debug = self.get_terrain_height_at(player_x + self.world_offset);
        let terrain_screen_y = terrain_y_debug - self.camera_y;
        self.ctx.set_fill_style_str("#ff0000");
        self.ctx.begin_path();
        self.ctx
            .arc(player_x, terrain_screen_y, 3.0, 0.0, std::f64::consts::PI * 2.0)
            .unwrap();
        self.ctx.fill();

        // Draw velocity indicator (line pointing in direction of movement)
        self.ctx.set_stroke_style_str("#333");
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();
        self.ctx.move_to(player_x, player_screen_y);
        self.ctx.line_to(
            player_x + (self.player_vx * 2.0),
            player_screen_y + (self.player_vy * 2.0),
        );
        self.ctx.stroke();

        // Draw UI
        self.draw_ui();
    }

    fn draw_terrain(&self) {
        self.ctx.set_stroke_style_str("#222");
        self.ctx.set_line_width(2.0);

        self.ctx.begin_path();

        let mut first = true;
        for segment in &self.hill_segments {
            let screen_x = segment.x - self.world_offset;
            let screen_y = segment.y - self.camera_y;
            let next_screen_y = segment.next_y - self.camera_y;

            // Only draw if visible on screen
            if screen_x > -100.0 && screen_x < self.canvas_width + 100.0 {
                if first {
                    self.ctx.move_to(screen_x, screen_y);
                    first = false;
                }

                let next_screen_x = screen_x + 5.0;
                self.ctx.line_to(next_screen_x, next_screen_y);
            }
        }

        self.ctx.stroke();

        // Draw ground line at bottom
        self.ctx.set_stroke_style_str("#888");
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();
        self.ctx.move_to(0.0, self.canvas_height - 20.0);
        self.ctx
            .line_to(self.canvas_width, self.canvas_height - 20.0);
        self.ctx.stroke();
    }

    fn draw_ui(&self) {
        self.ctx.set_font("16px monospace");
        self.ctx.set_fill_style_str("#000");

        let score_text = format!("Score: {}", self.score);
        self.ctx.fill_text(&score_text, 10.0, 30.0).unwrap_or(());

        let speed_text = format!("Speed: {:.1}", self.player_vx);
        self.ctx.fill_text(&speed_text, 10.0, 55.0).unwrap_or(());

        if self.game_over {
            self.ctx.set_font("bold 36px monospace");
            self.ctx.set_fill_style_str("#ff0000");
            self.ctx
                .fill_text(
                    "GAME OVER",
                    self.canvas_width / 2.0 - 100.0,
                    self.canvas_height / 2.0,
                )
                .unwrap_or(());
        } else {
            // Space indicator at bottom left
            let space_color = if self.space_pressed { "#0066ff" } else { "#cccccc" };
            self.ctx.set_fill_style_str(space_color);
            self.ctx.set_font("bold 14px monospace");
            self.ctx
                .fill_text("SPACE", 10.0, self.canvas_height - 35.0)
                .unwrap_or(());

            self.ctx.set_font("12px monospace");
            self.ctx.set_fill_style_str("#666");
            self.ctx
                .fill_text("Hold SPACE to thrust down", 10.0, self.canvas_height - 20.0)
                .unwrap_or(());
        }
    }

    fn get_terrain_height_at(&self, world_x: f64) -> f64 {
        // Calculate which segment we're in (segments are 5.0 units wide)
        let segment_index = (world_x / 5.0).floor() as usize;

        if segment_index < self.hill_segments.len() {
            let segment = &self.hill_segments[segment_index];
            let progress = (world_x - segment.x) / 5.0;
            return segment.y + (segment.next_y - segment.y) * progress;
        }

        self.canvas_height / 2.0 // Default height if nothing found
    }

    fn get_slope_at(&self, world_x: f64) -> f64 {
        // Calculate which segment we're in (segments are 5.0 units wide)
        let segment_index = (world_x / 5.0).floor() as usize;

        if segment_index < self.hill_segments.len() {
            return self.hill_segments[segment_index].slope;
        }

        0.0
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    Ok(())
}
