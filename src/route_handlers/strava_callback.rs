use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::strava_token_utils::get_token_from_code;
use crate::AppState;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum StravaCallbackParams {
    Success {
        code: String,
        scope: String,
        state: Option<String>,
    },
    Error {
        error: String,
        state: Option<String>,
    },
}

pub async fn handler(
    parameters: Query<StravaCallbackParams>,
    app_state: State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    match &parameters.0 {
        StravaCallbackParams::Error { error, state } => {
            debug!("Error: error = {}, state = {:?}", error, state);
            return axum::http::status::StatusCode::UNAUTHORIZED.into_response();
        }

        StravaCallbackParams::Success {
            code,
            scope: _,
            state: _,
        } => {
            match get_token_from_code(code.clone()).await {
                Ok(token) => {
                    let mut app_state = app_state.lock().await;
                    app_state.strava_token = Some(token);
                }

                Err(err) => {
                    error!("Failed to get strava token: {}", err);
                    return axum::http::status::StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            return axum::http::status::StatusCode::OK.into_response();
        }
    };
}
