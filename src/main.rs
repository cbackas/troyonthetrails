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
use crate::strava::beacon::Status;

mod db_service;
mod discord;
mod encryption;
mod env_utils;
mod route_handlers;
mod strava;
mod utils;

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

    tokio::spawn(async move {
        loop {
            let troy_status = db_service::get_troy_status().await;

            let beacon_data = match troy_status.beacon_url {
                Some(beacon_url) => match strava::beacon::get_beacon_data(beacon_url).await {
                    Ok(data) => Some(data),
                    Err(e) => {
                        tracing::error!("Failed to get beacon data: {}", e);
                        None
                    }
                },
                None => None,
            };

            let (activity_status, activity_id) = match beacon_data.clone() {
                Some(data) => (Some(data.status), data.activity_id),
                None => (None, None),
            };

            match activity_status {
                Some(Status::Active | Status::AutoPaused | Status::ManualPaused) => {
                    tracing::trace!("Beacon data indicates troy is active on the trails");
                    db_service::set_troy_status(true).await;
                    if !troy_status.is_on_trail {
                        tracing::info!("Troy status updated to on the trails");
                        discord::send_starting_webhook().await;
                    }
                }
                Some(Status::Uploaded) => {
                    tracing::info!("Beacon data indicates activity uploaded, clearing beacon url");
                    db_service::set_beacon_url(None).await;
                    if troy_status.is_on_trail {
                        db_service::set_troy_status(false).await;
                        discord::send_end_webhook(activity_id).await;
                    }
                }
                Some(Status::Dicarded) => {
                    tracing::info!("Beacon data indicates activity was discarded, clearing troy status and beacon url");
                    db_service::set_beacon_url(None).await;
                    if troy_status.is_on_trail {
                        db_service::set_troy_status(false).await;
                        discord::send_discard_webhook().await;
                    }
                }
                Some(Status::NotStarted) => {
                    tracing::info!("Beacon data indicates activity is not started yet");
                    let diff = {
                        let update_time = beacon_data.unwrap().update_time;
                        let update_time = update_time.datetime();
                        let now = chrono::Utc::now();
                        now - update_time
                    };
                    if diff.num_minutes() > 45 {
                        tracing::info!(
                            "Beacon data is old and activity never started, clearing beacon url"
                        );
                        db_service::set_beacon_url(None).await;
                    }
                }
                None => {}
                _ => {
                    tracing::warn!("Beacon data indicates unknown status");
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(45)).await;
        }
    });

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
