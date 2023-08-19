use std::{env, sync::Arc};

use dotenv::dotenv;

use anyhow::{Context, Error, Ok};
use axum::{
    routing::{get, post},
    Router,
};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod home_assistant;
mod route_handlers;

pub struct AppState {
    is_troy_on_the_trails: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "troyonthetrails=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("initializing app state");

    let app_state = Arc::new(Mutex::new(AppState {
        is_troy_on_the_trails: false,
    }));

    info!("initializing router");

    let wh_path = format!("/ws/trail-event/{:#}", get_ws_route()?);
    let assets_path = std::env::current_dir()?;
    let assets_path = format!("{}/assets", assets_path.to_str().unwrap());
    let app = Router::new()
        .route("/", get(route_handlers::home::handler))
        .route(
            &wh_path,
            post(route_handlers::webhooks::handler).get(route_handlers::webhooks::handler),
        )
        .nest(
            "/api",
            Router::new()
                .route("/trail-check", get(route_handlers::trail_check::handler))
                .route("/troy-check", get(route_handlers::troy_check::handler)),
        )
        .nest_service("/assets", ServeDir::new(assets_path))
        .with_state(app_state);

    // run it, make sure you handle parsing your environment variables properly!
    // let port = std::env::var("PORT").unwrap().parse::<u16>().unwrap();
    let port = 8080_u16;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    info!("router initialized, now listening on port {}", port);

    info!(
        "Server srarted, access the website via http://localhost:{}",
        port
    );
    info!(
        "Server srarted, sent trail status webhooks to http://localhost:{}{}",
        port, wh_path
    );

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("error while starting API server")?;
    Ok(())
}

fn get_ws_route() -> anyhow::Result<String> {
    let ws_seed = env::var("WS_SEED").context("Could not find WS_SEED in environment variables")?;

    // Create a Sha256 object
    let mut hasher = Sha256::new();

    // Write input message
    hasher.update(ws_seed);

    // Read hash digest and consume hasher
    let result = hasher.finalize();

    // Convert hash to a hex string and take the first 32 characters
    let hash_str = format!("{:x}", result);
    let short_hash = hash_str[0..32].to_string();

    Ok(short_hash)
}
