use axum::extract::Query;
use axum::response::IntoResponse;
use serde::Deserialize;
use tracing::{debug, error};

use crate::strava;

#[allow(dead_code)]
#[derive(Deserialize, Clone)]
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

pub async fn handler(parameters: Option<Query<StravaCallbackParams>>) -> impl IntoResponse {
    match parameters {
        Some(query) => match query.0 {
            StravaCallbackParams::Error { error, state: _ } => {
                debug!("Failed to authenticate Strava user: {}", error);
                super::html_template::HtmlTemplate(StravaCallbackTemplate {
                    message: "Failed to authenticate Strava user".to_string(),
                    detail: error.to_string(),
                    delayed_redirect: false,
                })
                .into_response()
            }

            StravaCallbackParams::Success {
                code,
                scope: _,
                state: _,
            } => {
                match strava::auth::get_token_from_code(code.clone()).await {
                    Ok(()) => {}
                    Err(err) => {
                        error!("Failed to get strava token: {}", err);
                        return super::html_template::HtmlTemplate(StravaCallbackTemplate {
                            message: "Failed to get Strava token".to_string(),
                            detail: err.to_string(),
                            delayed_redirect: false,
                        })
                        .into_response();
                    }
                };
                super::html_template::HtmlTemplate(StravaCallbackTemplate {
                    message: "Successfully authenticated Strava user".to_string(),
                    detail: "The page will automatically redirect ...".to_string(),
                    delayed_redirect: true,
                })
                .into_response()
            }
        },

        None => axum::response::Redirect::temporary("/").into_response(),
    }
}

#[derive(askama::Template)]
#[template(path = "pages/strava_callback.html")]
struct StravaCallbackTemplate {
    message: String,
    detail: String,
    delayed_redirect: bool,
}
