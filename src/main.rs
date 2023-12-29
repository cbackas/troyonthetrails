use std::sync::Arc;

use anyhow::Context;
use dotenv::dotenv;

use axum::{
    routing::{get, post},
    Router,
};
use tokio::{sync::Mutex, time::Instant};
use tower_http::services::{ServeDir, ServeFile};
use tracing::{debug, info};
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

use crate::strava_api_service::API_SERVICE;

mod db_service;
mod env_utils;
mod route_handlers;
mod strava_api_service;
mod utils;

#[derive(Default)]
pub struct AppState {
    // trail data
    trail_data_last_updated: Option<Instant>,
    trail_data: Vec<route_handlers::trail_check::TrailSystem>,
}
type SharedAppState = Arc<Mutex<AppState>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    debug!("initializing app state ...");

    {
        let mut strava_api_service = API_SERVICE.lock().await;
        strava_api_service.read_strava_auth_from_db().await;
    }

    let port = crate::env_utils::get_port();
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let host_uri = crate::env_utils::get_host_uri();

    info!("Starting server at host: {}", host_uri);

    axum::Server::bind(&addr)
        .serve(
            get_main_router()
                .with_state(SharedAppState::default())
                .into_make_service(),
        )
        .await
        .context("error while starting API server")?;

    debug!("Server srarted");

    anyhow::Ok(())
}

/**
 * main router for the app, defines basic root routes including the webhook event route
 * also brings together the other routers
 **/
fn get_main_router() -> Router<SharedAppState> {
    debug!("initializing router(s) ...");

    let wh_secret = crate::env_utils::get_webhook_secret();
    let wh_path = format!("/wh/trail-event/{:#}", wh_secret);
    info!("Webhook event route: {}", wh_path);

    let services_router = get_services_router();
    let api_router = get_api_router();
    Router::new()
        .route("/", get(route_handlers::home::handler))
        .route(
            &wh_path,
            post(route_handlers::webhooks::handler).get(route_handlers::webhooks::handler),
        )
        .route("/healthcheck", get(|| async { "Ok" }))
        .merge(services_router)
        .nest("/api", api_router)
}

/**
 * router for the static assets and such
**/
fn get_services_router() -> Router<SharedAppState> {
    let assets_path = match std::env::current_dir() {
        Ok(path) => path,
        Err(_) => std::path::PathBuf::from("./"),
    };

    let assets_path = format!("{}/assets", assets_path.to_str().unwrap());
    let favicon_path = format!("{}/favicon.ico", assets_path);
    let manifest_path = format!("{}/site.webmanifest", assets_path);

    Router::new()
        .nest_service("/assets", ServeDir::new(assets_path))
        .nest_service("/favicon.ico", ServeFile::new(favicon_path))
        .nest_service("/site.webmanifest", ServeFile::new(manifest_path))
}

/**
 * router for our api routes and the strava setup routes
 **/
fn get_api_router() -> Router<SharedAppState> {
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
        )
}
