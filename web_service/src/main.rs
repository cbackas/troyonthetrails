use anyhow::Context;
use dotenv::dotenv;

use axum::{
    http::{Request, Uri},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Router,
};
use tower_http::compression::{
    predicate::{DefaultPredicate, NotForContentType, Predicate},
    CompressionLayer,
};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

extern crate beacon_service;
extern crate shared_lib;

use shared_lib::env_utils;
use shared_lib::utils;

mod route_handlers;

struct RequestUri(Uri);

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

    tracing::debug!("initializing app state ...");

    {
        let db = db_service::get_db_service().await;
        db.init_tables().await;
    }

    beacon_service::beacon_loop::start();

    let port = crate::env_utils::get_port();
    let addr = format!("[::]:{port}")
        .parse::<std::net::SocketAddr>()
        .expect("unable to parse address");
    let host_uri = crate::env_utils::get_host_uri();

    tracing::info!("Starting server at host: {}", host_uri);

    let predicate = DefaultPredicate::new().and(NotForContentType::new("application/json"));
    let compression_layer = CompressionLayer::new().gzip(true).compress_when(predicate);

    axum::Server::bind(&addr)
        .serve(
            get_main_router()
                .layer(axum::middleware::from_fn(
                    |request: Request<_>, next: Next<_>| async move {
                        let uri = request.uri().clone();

                        let mut response = next.run(request).await;

                        response.extensions_mut().insert(RequestUri(uri));

                        response
                    },
                ))
                .layer(TraceLayer::new_for_http().on_response(
                    |response: &Response, latency: std::time::Duration, _span: &tracing::Span| {
                        let url = match response.extensions().get::<RequestUri>().map(|r| &r.0) {
                            Some(uri) => uri.to_string(),
                            None => "unknown".to_string(),
                        };
                        let status = response.status();
                        let latency = utils::duration_to_ms_string(latency);

                        if url == "/healthcheck" {
                            tracing::trace!("{} {} {}", url, status, latency);
                            return;
                        }

                        tracing::debug!("{} {} {}", url, status, latency);
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
fn get_main_router() -> Router {
    tracing::debug!("initializing router(s) ...");

    let wh_secret = crate::env_utils::get_webhook_secret();
    let wh_path = format!("/wh/trail-event/{wh_secret:#}");
    tracing::info!("Webhook event route: {}", wh_path);

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
fn get_services_router() -> Router {
    let assets_path = match std::env::current_dir() {
        Ok(path) => path,
        Err(_) => std::path::PathBuf::from("./"),
    };

    let assets_path = format!("{}/assets", assets_path.to_str().unwrap());
    let favicon_path = format!("{assets_path}/favicon.ico");
    let manifest_path = format!("{assets_path}/site.webmanifest");

    Router::new()
        .nest_service("/assets", ServeDir::new(assets_path))
        .nest_service("/favicon.ico", ServeFile::new(favicon_path))
        .nest_service("/site.webmanifest", ServeFile::new(manifest_path))
}

/**
 * router for our api routes and the strava setup routes
 **/
fn get_api_router() -> Router {
    Router::new()
        .route("/trail-check", get(route_handlers::trail_check::handler))
        .route(
            "/trail-ride-counts",
            get(route_handlers::trail_ride_counts::handler),
        )
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
