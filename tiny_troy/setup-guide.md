# Tiny Rider WASM Game - Setup Guide

## Project Structure

```
your-rust-project/
├── Cargo.toml
├── src/
│   └── main.rs (your Axum server)
├── game/                          # NEW: Game crate
│   ├── Cargo.toml                 # Use the provided Cargo.toml
│   └── src/
│       └── lib.rs                 # Use the provided lib.rs
├── static/                        # NEW: Compiled WASM goes here
│   └── (will be generated)
└── game-index.html                # Save this in your templates/ or static/
```

## Setup Steps

### 1. Create the game crate
```bash
# In your project root
cd game
# Copy the provided Cargo.toml and src/lib.rs here
```

### 2. Build the WASM
```bash
# Install wasm-pack (if not already installed)
cargo install wasm-pack

# From project root, build the game
wasm-pack build game --release --target web --out-dir ../static/game
```

This generates:
- `static/game/game.js` - The glue code
- `static/game/game_bg.wasm` - The actual game binary
- `static/game/game.d.ts` - Type definitions

### 3. Update your Axum routes

Add to your main Axum server code:

```rust
use tower_http::services::ServeDir;
use axum::Router;

let app = Router::new()
    // ... your other routes ...
    .route("/game", axum::routing::get(|| async {
        axum::response::Html(include_str!("../static/game-index.html"))
    }))
    .nest_service("/static", ServeDir::new("static"));
```

### 4. Serve the HTML

Copy `game-index.html` to your static directory or embed it in your Axum route.

Update the import path in the HTML if needed:
```javascript
import init, { Game } from '/static/game/game.js';  // Adjust path as needed
```

## Build & Deploy

```bash
# Build everything
cargo build --release

# Run your server
cargo run --release

# Visit http://localhost:YOUR_PORT/game
```

## How It Works

- **Player starts at 25% screen X**, top of the screen
- **Gravity pulls down** continuously
- **Space bar thrusts downward** (accelerates toward terrain)
- **Hills scroll from right to left** as you build speed
- **Score increases** based on distance traveled
- **Difficulty increases** - hills get steeper and more complex
- **Game over** if you fall off the bottom or go too high

## Customization

Open `game/src/lib.rs` to tweak:

- `GRAVITY: f64 = 0.6` - How fast gravity accelerates you
- `THRUST_FORCE: f64 = -0.15` - How much space bar pushes you
- `FRICTION: f64 = 0.98` - Velocity decay per frame
- Hill terrain generation in `generate_terrain()` - Adjust amplitude, segment width, etc.
- Player visuals in `draw_ui()` - Change colors, shapes, or add trails

## Performance Notes

WASM runs at near-native speed and easily handles:
- 60fps game loop
- Smooth physics simulation
- Canvas rendering
- Input handling

Binary size is ~100KB (gzipped ~30KB), so deployment is fast.

## Troubleshooting

**"No canvas element" error:**
- Check that HTML has `<canvas id="gameCanvas"></canvas>`

**CORS issues with WASM:**
- Make sure WASM is served from the same origin
- Check Content-Type headers are set correctly (application/wasm)

**Can't find module:**
- Verify the import path in HTML matches where wasm-pack outputs files
- Default: `/static/game.js` (adjust if different structure)
