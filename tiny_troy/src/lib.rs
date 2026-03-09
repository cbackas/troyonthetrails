use js_sys::Math;
use wasm_bindgen::prelude::*;
use web_sys::{window, CanvasRenderingContext2d, HtmlCanvasElement};
use rapier2d::prelude::*;

const CANVAS_WIDTH: f64 = 800.0;
const CANVAS_HEIGHT: f64 = 600.0;
const PLAYER_X_RATIO: f64 = 0.25; // 25% from left

#[wasm_bindgen]
pub struct Game {
    ctx: CanvasRenderingContext2d,
    canvas_width: f64,
    canvas_height: f64,

    // Rapier2D physics
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    player_handle: RigidBodyHandle,

    // World state
    world_offset: f64, // How far we've scrolled (X)
    camera_y: f64,     // Camera vertical position (follows player)
    time: f64,
    score: u32,
    game_over: bool,
    paused: bool,

    // Input
    space_pressed: bool,

    // Terrain (for rendering)
    hill_segments: Vec<HillSegment>,
    terrain_seed: f64,

    // Adjustable physics settings
    gravity_y: f64,
    thrust_force: f64,
    downhill_accel: f64,  // Not used with rapier but keep for compatibility
    launch_strength: f64, // Not used with rapier but keep for compatibility
    slope_multiplier: f64, // Multiplier for slope-based acceleration on rails
}

#[derive(Clone)]
struct HillSegment {
    x: f64,
    y: f64,
    next_y: f64,
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

        // Generate random seed for terrain
        let terrain_seed = (Math::random() * 10000.0).floor();

        // Initialize rapier2d physics
        let mut rigid_body_set = RigidBodySet::new();
        let mut collider_set = ColliderSet::new();

        // Create player as a dynamic ball
        // Start at the top of the steep ramp (center_y - 150 = ~150 for 600px canvas)
        let start_y = (canvas_height / 2.0) - 150.0;
        let player_body = RigidBodyBuilder::dynamic()
            .translation(vector![100.0, start_y as f32])
            .linvel(vector![50.0, 30.0]) // Strong horizontal start with some downward momentum
            .ccd_enabled(true) // Enable continuous collision detection to prevent tunneling
            .build();
        let player_handle = rigid_body_set.insert(player_body);

        // Player collider (ball with radius 6.0)
        let player_collider = ColliderBuilder::ball(6.0)
            .restitution(0.3) // Some bounce
            .friction(0.5)
            .active_events(ActiveEvents::COLLISION_EVENTS) // Enable collision events
            .build();
        collider_set.insert_with_parent(player_collider, player_handle, &mut rigid_body_set);

        let mut game = Game {
            ctx,
            canvas_width,
            canvas_height,
            rigid_body_set,
            collider_set,
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            player_handle,
            world_offset: 0.0,
            camera_y: 0.0,
            time: 0.0,
            score: 0,
            game_over: false,
            paused: false,
            space_pressed: false,
            hill_segments: Vec::new(),
            terrain_seed,
            gravity_y: 9.81 * 20.0, // Much stronger gravity for faster gameplay
            thrust_force: 750.0, // Strong thrust for noticeable effect (2.5 * 300)
            downhill_accel: 0.3,
            launch_strength: 0.12,
            slope_multiplier: 5.0,
        };

        game.generate_terrain();
        Ok(game)
    }

    fn generate_terrain(&mut self) {
        self.hill_segments.clear();

        let mut x = 0.0;
        let center_y = self.canvas_height / 2.0;
        let segment_width = 5.0;

        // Use seed to randomize wave frequencies
        let freq_offset1 = (self.terrain_seed * 0.123).sin() * 0.01;
        let freq_offset2 = (self.terrain_seed * 0.456).sin() * 0.02;
        let freq_offset3 = (self.terrain_seed * 0.789).sin() * 0.005;

        // Track previous sampled point for creating continuous collider segments
        let mut prev_collider_x: Option<f32> = None;
        let mut prev_collider_y: Option<f32> = None;

        // Generate smooth sine-wave terrain like Tiny Wings
        for i in 0..20000 {
            // Progressive difficulty
            let progression = (i as f64) / 2000.0;

            // Multiple sine waves at different frequencies create natural hills
            // Add seed offsets to vary the terrain each game
            let freq1 = (i as f64 + self.terrain_seed) * (0.02 + freq_offset1);
            let freq2 = (i as f64 + self.terrain_seed * 0.5) * (0.05 + freq_offset2);
            let freq3 = (i as f64 + self.terrain_seed * 0.3) * (0.01 + freq_offset3);

            // Combine waves with different amplitudes
            let wave = freq1.sin() * 0.5 + freq2.sin() * 0.3 + freq3.sin() * 0.2;

            // Varying depth creates interesting terrain (increased for deeper valleys)
            let base_depth = 150.0 + (progression * 150.0).min(300.0);

            // Strong downward bias at the start (first ~1000px), fading out
            let start_downward_bias = if i < 300 {
                // First 1500px: strong downward slope
                let start_progress = i as f64 / 300.0; // 0.0 to 1.0
                // Start high, force downward trend
                let initial_offset = -150.0 * (1.0 - start_progress); // -150 fading to 0
                let downward_force = 350.0 * start_progress; // 0 growing to 350
                initial_offset + downward_force
            } else {
                // After start: gradual downward trend continues
                let downward_bias = (i as f64 - 300.0) * 0.15;
                350.0 + downward_bias
            };

            let y = center_y + wave * base_depth + start_downward_bias;

            // Calculate next point
            let next_i = i + 1;
            let next_progression = (next_i as f64) / 2000.0;
            let next_freq1 = (next_i as f64 + self.terrain_seed) * (0.02 + freq_offset1);
            let next_freq2 = (next_i as f64 + self.terrain_seed * 0.5) * (0.05 + freq_offset2);
            let next_freq3 = (next_i as f64 + self.terrain_seed * 0.3) * (0.01 + freq_offset3);
            let next_wave =
                next_freq1.sin() * 0.5 + next_freq2.sin() * 0.3 + next_freq3.sin() * 0.2;

            let next_base_depth = 150.0 + (next_progression * 150.0).min(300.0);

            let next_start_downward_bias = if next_i < 300 {
                let start_progress = next_i as f64 / 300.0;
                let initial_offset = -150.0 * (1.0 - start_progress);
                let downward_force = 350.0 * start_progress;
                initial_offset + downward_force
            } else {
                let downward_bias = (next_i as f64 - 300.0) * 0.15;
                350.0 + downward_bias
            };

            let next_y = center_y + next_wave * next_base_depth + next_start_downward_bias;

            self.hill_segments.push(HillSegment {
                x,
                y,
                next_y,
            });

            // Create continuous terrain colliders by sampling every 2 segments (10px)
            // This gives us 10000 colliders but ensures no gaps
            if i % 2 == 0 {
                if let (Some(px), Some(py)) = (prev_collider_x, prev_collider_y) {
                    // Create segment from previous sampled point to current point
                    let segment_collider = ColliderBuilder::segment(
                        point![px, py],
                        point![x as f32, y as f32]
                    )
                    .friction(0.8)
                    .restitution(0.1)
                    .build();
                    self.collider_set.insert(segment_collider);
                }
                // Update previous point
                prev_collider_x = Some(x as f32);
                prev_collider_y = Some(y as f32);
            }

            x += segment_width;
        }
    }

    pub fn set_space_pressed(&mut self, pressed: bool) {
        self.space_pressed = pressed;
    }

    #[wasm_bindgen]
    pub fn set_gravity(&mut self, value: f64) {
        self.gravity_y = value * 200.0; // Scale up significantly for faster gameplay
    }

    #[wasm_bindgen]
    pub fn set_thrust_force(&mut self, value: f64) {
        self.thrust_force = value * 300.0; // Scale up for much more noticeable thrust
    }

    #[wasm_bindgen]
    pub fn set_downhill_accel(&mut self, value: f64) {
        self.downhill_accel = value;
    }

    #[wasm_bindgen]
    pub fn set_launch_strength(&mut self, value: f64) {
        self.launch_strength = value;
    }

    #[wasm_bindgen]
    pub fn set_slope_multiplier(&mut self, value: f64) {
        self.slope_multiplier = value;
    }

    #[wasm_bindgen]
    pub fn reset(&mut self) {
        // Reset player physics body
        if let Some(player) = self.rigid_body_set.get_mut(self.player_handle) {
            // Start at the top of the steep ramp
            let start_y = (self.canvas_height / 2.0) - 150.0;
            player.set_translation(vector![100.0, start_y as f32], true);
            player.set_linvel(vector![50.0, 30.0], true); // Strong horizontal start with some downward momentum
            player.set_angvel(0.0, true);
        }

        self.world_offset = 0.0;
        self.camera_y = 0.0;
        self.time = 0.0;
        self.score = 0;
        self.game_over = false;
        self.paused = false;
        self.space_pressed = false;

        // Clear old terrain colliders
        let colliders_to_remove: Vec<_> = self.collider_set.iter().map(|(h, _)| h).collect();
        for handle in colliders_to_remove {
            self.collider_set.remove(handle, &mut self.island_manager, &mut self.rigid_body_set, false);
        }

        // Re-add player collider
        let player_collider = ColliderBuilder::ball(6.0)
            .restitution(0.3)
            .friction(0.5)
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .build();
        self.collider_set.insert_with_parent(player_collider, self.player_handle, &mut self.rigid_body_set);

        // Generate new terrain
        self.terrain_seed = (Math::random() * 10000.0).floor();
        self.generate_terrain();
    }

    #[wasm_bindgen]
    pub fn toggle_pause(&mut self) {
        if !self.game_over {
            self.paused = !self.paused;
        }
    }

    #[wasm_bindgen]
    pub fn is_game_over(&self) -> bool {
        self.game_over
    }

    pub fn update(&mut self) {
        if self.game_over || self.paused {
            return;
        }

        self.time += 1.0;

        // Get player state (copy values to avoid borrow checker issues)
        let (pos, mut vel, world_x) = if let Some(player) = self.rigid_body_set.get(self.player_handle) {
            let pos = *player.translation();
            let vel = *player.linvel();
            let world_x = pos.x as f64;
            (pos, vel, world_x)
        } else {
            return;
        };

        let _player_x = self.canvas_width * PLAYER_X_RATIO;
        let player_radius = 6.0;

        // Get terrain info at player position
        let terrain_y = self.get_terrain_y_at(world_x);
        let slope = self.get_terrain_slope_at(world_x);

        // Pre-calculate new position and terrain for both states
        let new_x = pos.x + vel.x * (1.0 / 120.0);
        let new_world_x = new_x as f64;
        let new_terrain_y = self.get_terrain_y_at(new_world_x);
        let max_y = new_terrain_y - player_radius; // Don't go below terrain

        // Check if grounded (within small distance of terrain)
        let distance_to_terrain = (pos.y as f64) - (terrain_y - player_radius);

        // At peaks/crests (slope near 0), allow natural flight if moving fast
        // On steep sections, stick to terrain more aggressively
        let at_peak = slope.abs() < 0.1;
        let high_velocity = vel.x > 80.0;
        let should_fly_at_peak = at_peak && high_velocity;

        let is_grounded = distance_to_terrain.abs() < 10.0 && vel.y >= -5.0 && !should_fly_at_peak;

        // Now get mutable access to player for updates
        if let Some(player) = self.rigid_body_set.get_mut(self.player_handle) {

            if is_grounded {
                // GROUNDED: Ride the rails (Tiny Wings style)
                // Apply slope-based acceleration
                // Downhill (positive slope in our coords) = gain speed
                // Uphill (negative slope) = lose speed
                let slope_accel = slope * self.downhill_accel * self.slope_multiplier;

                // Only apply deceleration if already slow - preserve landing momentum
                if vel.x > 50.0 || slope_accel > 0.0 {
                    // Keep speed or accelerate
                    vel.x += slope_accel as f32;
                } else {
                    // Apply limited deceleration when slow
                    let clamped_accel = slope_accel.max(-2.0);
                    vel.x += clamped_accel as f32;
                }

                // No minimum speed - let natural deceleration lead to game over

                // Lock to terrain surface at new position
                let target_y = new_terrain_y - player_radius;
                player.set_translation(vector![new_x, target_y as f32], true);

                // Stick to terrain - zero vertical velocity
                vel.y = 0.0;

                player.set_linvel(vel, true);

                // Award points when riding
                self.score += 1;
            } else {
                // IN AIR: Normal physics
                // Apply gravity
                vel.y += self.gravity_y as f32 * (1.0 / 120.0);

                // Apply thrust downward when space is pressed
                if self.space_pressed {
                    vel.y += self.thrust_force as f32 * (1.0 / 120.0);
                }

                // Air drag
                vel.x *= 0.995;
                vel.y *= 0.98;

                // Update position based on velocity
                let new_y = pos.y + vel.y * (1.0 / 120.0);

                // Don't let player go below terrain when falling
                let clamped_y = new_y.min(max_y as f32);

                player.set_translation(vector![new_x, clamped_y], true);
                player.set_linvel(vel, true);
            }
        }

        // Update world offset
        self.world_offset = pos.x as f64 - self.canvas_width * PLAYER_X_RATIO;

        // Update camera
        let player_y = pos.y as f64;
        let target_screen_y = self.canvas_height * 0.4;
        let target_camera_y = player_y - target_screen_y;
        let camera_smoothing = 0.15;
        self.camera_y += (target_camera_y - self.camera_y) * camera_smoothing;

        // Check game over conditions
        // Only lose if grounded and too slow
        if is_grounded && vel.x < 30.0 {
            self.game_over = true;
        }

        let player_screen_y = player_y - self.camera_y;
        if player_screen_y > self.canvas_height + 100.0 || player_screen_y < -100.0 {
            self.game_over = true;
        }
    }

    // Helper to get terrain height at a world X position
    fn get_terrain_y_at(&self, world_x: f64) -> f64 {
        let segment_index = ((world_x / 5.0).floor() as usize).min(self.hill_segments.len().saturating_sub(1));
        if segment_index < self.hill_segments.len() {
            let segment = &self.hill_segments[segment_index];
            let local_x = world_x - segment.x;
            let progress = (local_x / 5.0).clamp(0.0, 1.0);
            segment.y + (segment.next_y - segment.y) * progress
        } else {
            self.canvas_height / 2.0
        }
    }

    // Helper to get terrain slope at a world X position
    fn get_terrain_slope_at(&self, world_x: f64) -> f64 {
        let segment_index = ((world_x / 5.0).floor() as usize).min(self.hill_segments.len().saturating_sub(1));
        if segment_index < self.hill_segments.len() {
            let segment = &self.hill_segments[segment_index];
            (segment.next_y - segment.y) / 5.0 // Rise over run
        } else {
            0.0
        }
    }

    pub fn render(&self) {
        // Clear canvas
        self.ctx.set_fill_style_str("#f0f0f0");
        self.ctx
            .fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

        // Draw terrain
        self.draw_terrain();

        // Get player position and velocity from physics
        if let Some(player) = self.rigid_body_set.get(self.player_handle) {
            let pos = player.translation();
            let vel = player.linvel();

            // Draw player (simple circle for minimalist style)
            let player_x = self.canvas_width * PLAYER_X_RATIO;
            let player_screen_y = pos.y as f64 - self.camera_y;
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

            // Draw velocity indicator (line pointing in direction of movement)
            self.ctx.set_stroke_style_str("#333");
            self.ctx.set_line_width(1.0);
            self.ctx.begin_path();
            self.ctx.move_to(player_x, player_screen_y);
            self.ctx.line_to(
                player_x + (vel.x as f64 * 2.0),
                player_screen_y + (vel.y as f64 * 2.0),
            );
            self.ctx.stroke();
        }

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

        // Get player data from physics
        if let Some(player) = self.rigid_body_set.get(self.player_handle) {
            let pos = player.translation();
            let vel = player.linvel();

            // Top left stats
            let score_text = format!("Score: {}", self.score);
            self.ctx.fill_text(&score_text, 10.0, 30.0).unwrap_or(());

            let speed_text = format!("Speed: {:.1}", vel.x);
            self.ctx.fill_text(&speed_text, 10.0, 55.0).unwrap_or(());

            let x_pos_text = format!("X: {:.0}", self.world_offset);
            self.ctx.fill_text(&x_pos_text, 10.0, 80.0).unwrap_or(());

            let y_pos_text = format!("Y: {:.0}", pos.y);
            self.ctx.fill_text(&y_pos_text, 10.0, 105.0).unwrap_or(());

            let vx_text = format!("VX: {:.2}", vel.x);
            self.ctx.fill_text(&vx_text, 10.0, 130.0).unwrap_or(());

            // Flip VY sign for display (negative internal = up, show as positive)
            let vy_text = format!("VY: {:.2}", -vel.y);
            self.ctx.fill_text(&vy_text, 10.0, 155.0).unwrap_or(());
        }

        if self.paused {
            self.ctx.set_font("bold 36px monospace");
            self.ctx.set_fill_style_str("#666");
            self.ctx
                .fill_text(
                    "PAUSED",
                    self.canvas_width / 2.0 - 80.0,
                    self.canvas_height / 2.0,
                )
                .unwrap_or(());

            self.ctx.set_font("16px monospace");
            self.ctx
                .fill_text(
                    "Press ESC to resume",
                    self.canvas_width / 2.0 - 90.0,
                    self.canvas_height / 2.0 + 40.0,
                )
                .unwrap_or(());
        } else if self.game_over {
            self.ctx.set_font("bold 36px monospace");
            self.ctx.set_fill_style_str("#ff0000");
            self.ctx
                .fill_text(
                    "GAME OVER",
                    self.canvas_width / 2.0 - 100.0,
                    self.canvas_height / 2.0,
                )
                .unwrap_or(());

            self.ctx.set_font("16px monospace");
            self.ctx.set_fill_style_str("#666");
            self.ctx
                .fill_text(
                    "Press SPACE or R to restart",
                    self.canvas_width / 2.0 - 120.0,
                    self.canvas_height / 2.0 + 40.0,
                )
                .unwrap_or(());
        } else {
            // Space indicator at bottom left
            let space_color = if self.space_pressed {
                "#0066ff"
            } else {
                "#cccccc"
            };
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

}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    Ok(())
}
