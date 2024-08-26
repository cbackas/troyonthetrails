use std::sync::Arc;

use anyhow::Context;
use dotenv::dotenv;

use axum::{
    http::{Request, Uri},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Router,
};
use tokio::{
    sync::{Mutex, OnceCell},
    time::Instant,
};
use tower_http::compression::{
    predicate::{DefaultPredicate, NotForContentType, Predicate},
    CompressionLayer,
};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{debug, info, trace, Span};
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

use crate::db_service::DbService;

extern crate shared_utils;

use shared_utils::env_utils;
use shared_utils::utils;

mod beacon_loop;
mod db_service;
mod discord;
mod encryption;
mod route_handlers;
mod strava;

pub static DB_SERVICE: OnceCell<DbService> = OnceCell::const_new();

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
        let db = db_service::get_db_service().await;
        db.init_tables().await;
    }

    run_beacon_loop();

    let port = crate::env_utils::get_port();
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let host_uri = crate::env_utils::get_host_uri();

    info!("Starting server at host: {}", host_uri);

    let predicate = DefaultPredicate::new().and(NotForContentType::new("application/json"));
    let compression_layer = CompressionLayer::new().gzip(true).compress_when(predicate);

    axum::Server::bind(&addr)
        .serve(
            get_main_router()
                .with_state(SharedAppState::default())
                .layer(axum::middleware::from_fn(uri_middleware))
                .layer(TraceLayer::new_for_http().on_response(
                    |response: &Response, latency: std::time::Duration, _span: &Span| {
                        let url = match response.extensions().get::<RequestUri>().map(|r| &r.0) {
                            Some(uri) => uri.to_string(),
                            None => "unknown".to_string(),
                        };
                        let status = response.status();
                        let latency = utils::duration_to_ms_string(latency);

                        if url == "/healthcheck" {
                            trace!("{} {} {}", url, status, latency);
                            return;
                        }

                        debug!("{} {} {}", url, status, latency);
                    },
                ))
                .layer(compression_layer)
                .into_make_service(),
        )
        .await
        .context("error while starting API server")?;

    Ok(())
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

struct RequestUri(Uri);

async fn uri_middleware<B>(request: Request<B>, next: Next<B>) -> Response {
    let uri = request.uri().clone();

    let mut response = next.run(request).await;

    response.extensions_mut().insert(RequestUri(uri));

    response
}

// loop that continuously checks the db for a beacon url and processes the data if found
fn run_beacon_loop() {
    match (std::env::var("FLY_REGION"), std::env::var("PRIMARY_REGION")) {
        (Ok(fly_region), Ok(primary_region)) => {
            if fly_region == primary_region {
                tracing::info!("Beacon loop running in region: {}", fly_region);
            } else {
                tracing::trace!(
                    "Fly region ({}) and primary region ({}) do not match, skipping beacon loop",
                    fly_region,
                    primary_region
                );
                return;
            }
        }
        _ => {
            tracing::warn!("FLY_REGION and PRIMARY_REGION are not both set, running beacon loop");
        }
    }

    tokio::spawn(async move {
        loop {
            beacon_loop::process_beacon().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(45)).await;
        }
    });
}
