use anyhow::Context;
use dotenv::dotenv;

use axum::{
    http::{Request, Uri},
    middleware::Next,
    response::Response,
    routing::get,
    Router,
};
use tower_http::{
    compression::{
        predicate::{DefaultPredicate, NotForContentType, Predicate},
        CompressionLayer,
    },
    trace::TraceLayer,
};
use tracing::Span;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

extern crate shared_lib;

use shared_lib::env_utils;
use shared_lib::utils;

mod browser;
mod handler;
mod html_template;

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

    let port = 7070;
    let addr = format!("[::]:{}", port)
        .parse::<std::net::SocketAddr>()
        .expect("unable to parse address");
    // TODO make the host_uri reflect the correct port
    let host_uri = crate::env_utils::get_host_uri();

    tracing::info!("Starting server at host: {}", host_uri);

    let predicate = DefaultPredicate::new().and(NotForContentType::new("application/json"));
    let compression_layer = CompressionLayer::new().gzip(true).compress_when(predicate);

    axum::Server::bind(&addr)
        .serve(
            get_main_router()
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

    Router::new()
        .route("/", get(handler::route_handler))
        .route("/assets/index.js", get(handler::js_handler))
        .route("/assets/index.css", get(handler::css_handler))
        .route("/healthcheck", get(|| async { "Ok" }))
}

struct RequestUri(Uri);

async fn uri_middleware<B>(request: Request<B>, next: Next<B>) -> Response {
    let uri = request.uri().clone();

    let mut response = next.run(request).await;

    response.extensions_mut().insert(RequestUri(uri));

    response
}
