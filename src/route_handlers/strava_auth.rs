use axum::response::IntoResponse;
use tracing::{debug, error};

pub async fn handler() -> impl IntoResponse {
    let client_id = match std::env::var("STRAVA_CLIENT_ID") {
        Ok(id) => id,
        Err(err) => {
            error!("STRAVA_CLIENT_ID environment variable not found: {}", err);
            return axum::http::status::StatusCode::BAD_REQUEST.into_response();
        }
    };

    // get teh HOST env var which is probably troyonthetrails.com
    let host_uri = match std::env::var("HOST") {
        Ok(host) => format!("https://{}", host),
        _ => "http://localhost:8080".to_string(),
    };
    let host_uri = host_uri.as_bytes();
    let host_uri = url::form_urlencoded::byte_serialize(host_uri);
    let host_uri: String = host_uri.collect();

    let mut auth_url = String::with_capacity(200);
    auth_url.push_str("https://www.strava.com/oauth/authorize");
    auth_url.push_str(&format!("?client_id={}", client_id));
    auth_url.push_str("&response_type=code");
    auth_url.push_str(&format!("&redirect_uri={}/api/strava/callback", host_uri));
    auth_url.push_str("&approval_prompt=force");
    auth_url.push_str("&scope=read,activity:read");

    debug!("Redirecting user to strava auth url, {}", auth_url);

    axum::response::Redirect::temporary(&auth_url).into_response()
}
