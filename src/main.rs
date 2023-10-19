use std::{env, sync::Arc};

use dotenv::dotenv;

use anyhow::Context;
use axum::{
    routing::{get, post},
    Router,
};
use sha2::{Digest, Sha256};
use tokio::{sync::Mutex, time::Instant};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod env_utils;
mod route_handlers;
mod strava_token_utils;

pub struct AppState {
    // troy data
    is_troy_on_the_trails: bool,
    troy_status_last_updated: Option<Instant>,
    // trail data
    trail_data_last_updated: Option<Instant>,
    trail_data: Vec<route_handlers::trail_check::TrailSystem>,
    // stava data
    strava_token: Option<strava_token_utils::TokenData>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let log_level = match std::env::var("LOG_LEVEL") {
        Ok(level) => match level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => level,
            _ => "info".to_string(),
        },
        Err(_) => "info".to_string(),
    };
    let tracing_filter = format!("troyonthetrails={}", log_level);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("initializing app state");

    let strava_token = match crate::strava_token_utils::read_token_data_from_file().await {
        Ok(token) => Some(token),
        Err(_) => None,
    };

    let app_state = Arc::new(Mutex::new(AppState {
        is_troy_on_the_trails: false,
        troy_status_last_updated: None,
        trail_data_last_updated: None,
        trail_data: vec![],
        strava_token,
    }));

    info!("initializing router");

    let wh_path = format!("/wh/trail-event/{:#}", get_wh_route()?);
    let assets_path = std::env::current_dir()?;
    let assets_path = format!("{}/assets", assets_path.to_str().unwrap());
    let favicon_path = format!("{}/favicon.ico", assets_path);
    let manifest_path = format!("{}/site.webmanifest", assets_path);
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
                .route("/troy-check", get(route_handlers::troy_check::handler))
                .nest(
                    "/strava",
                    Router::new()
                        .route(
                            "/supersecretauthroute",
                            get(route_handlers::strava_auth::handler),
                        )
                        .route("/callback", get(route_handlers::strava_callback::handler))
                        .route("/data", get(route_handlers::strava_data::handler)),
                ),
        )
        .nest_service("/assets", ServeDir::new(assets_path))
        .nest_service("/favicon.ico", ServeFile::new(favicon_path))
        .nest_service("/site.webmanifest", ServeFile::new(manifest_path))
        .with_state(app_state);

    let port = crate::env_utils::get_port();
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let host_uri = crate::env_utils::get_host_uri(Some(port));

    info!("Server srarted, access the website via {}", host_uri);
    info!(
        "Server srarted, sent trail status webhooks to {}{}",
        host_uri, wh_path
    );

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("error while starting API server")?;

    anyhow::Ok(())
}

fn get_wh_route() -> anyhow::Result<String> {
    let ws_seed = env::var("WH_SEED").context("Could not find WH_SEED in environment variables")?;

    // Create a Sha256 object
    let mut hasher = Sha256::new();

    // Write input message
    hasher.update(ws_seed);

    // Read hash digest and consume hasher
    let result = hasher.finalize();

    // Convert hash to a hex string and take the first 32 characters
    let hash_str = format!("{:x}", result);
    let short_hash = hash_str[0..32].to_string();

    anyhow::Ok(short_hash)
}
