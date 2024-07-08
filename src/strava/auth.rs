use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::db_service;

use super::api_service::Athlete;

#[derive(Deserialize, Debug, Clone)]
pub struct StravaTokenResponse {
    pub _token_type: String,
    pub expires_at: u64,
    pub _expires_in: u64,
    pub refresh_token: String,
    pub access_token: String,
    pub athlete: Option<Athlete>,
}

impl From<StravaTokenResponse> for TokenData {
    fn from(val: StravaTokenResponse) -> Self {
        TokenData {
            expires_at: val.expires_at,
            access_token: val.access_token,
            refresh_token: val.refresh_token,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenData {
    pub expires_at: u64,
    pub access_token: String,
    pub refresh_token: String,
}

static TOKEN_DATA: OnceCell<Option<TokenData>> = OnceCell::const_new();
pub async fn get_token() -> Option<TokenData> {
    let token_data = TOKEN_DATA
        .get_or_init(|| async {
            match db_service::get_strava_auth().await {
                Some(data) => Some(data),
                _ => {
                    tracing::warn!("No strava auth data found in db");
                    None
                }
            }
        })
        .await;

    if let Some(data) = token_data {
        if data.expires_at >= chrono::Utc::now().timestamp() as u64 {
            return Some(data.clone());
        } else {
            tracing::warn!("Strava token has expired");
        }
    } else {
        return None;
    }

    let token_data = token_data.clone().expect("No token found");

    let token_data = get_token_from_refresh(token_data.refresh_token)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to refresh strava token: {}", e.to_string()));

    let token_data = match token_data {
        Ok(token_data) => Some(token_data),
        Err(e) => {
            tracing::error!("{}", e);
            None
        }
    };

    let _ = TOKEN_DATA.set(token_data.clone());

    token_data
}

pub async fn get_token_from_code(code: String) -> anyhow::Result<()> {
    let client_id = std::env::var("STRAVA_CLIENT_ID")
        .context("STRAVA_CLIENT_ID environment variable not found")?;
    let client_secret = std::env::var("STRAVA_CLIENT_SECRET")
        .context("STRAVA_CLIENT_SECRET environment variable not found")?;

    tracing::debug!("Fetching new strava token using OAuth flow");

    let client = reqwest::Client::new();
    let resp = client
        .post("https://www.strava.com/api/v3/oauth/token")
        .query(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("grant_type", "authorization_code".to_string()),
        ])
        .send()
        .await
        .context("Failed to get token from strava")?;

    if resp.status().is_success() {
        let strava_data = resp.text().await;
        let strava_data: StravaTokenResponse =
            serde_json::from_str(&strava_data.unwrap()).context("Failed to deserialize JSON")?;

        // if strava_data has an athlete then compare the id to the one in the env var
        if let Some(athlete) = strava_data.clone().athlete {
            let strava_user_id = match std::env::var("STRAVA_USER_ID").ok() {
                Some(strava_user_id) => strava_user_id,
                None => {
                    return Err(anyhow::anyhow!(
                        "Successfully authenticated Strava user but no STRAVA_USER_ID defined"
                    ))
                }
            };
            if athlete.id.to_string().as_str() != strava_user_id {
                return Err(anyhow::anyhow!(
                    "Successfully authenticated Strava user but the user id does not match the defined STRAVA_USER_ID"
                ));
            }
        }

        let strava_data: TokenData = strava_data.into();

        let _ = TOKEN_DATA.set(Some(strava_data.clone()));

        db_service::set_strava_auth(strava_data).await;

        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}

async fn get_token_from_refresh(refresh_token: String) -> anyhow::Result<TokenData> {
    let client_id = std::env::var("STRAVA_CLIENT_ID")
        .context("STRAVA_CLIENT_ID environment variable not found")?;
    let client_secret = std::env::var("STRAVA_CLIENT_SECRET")
        .context("STRAVA_CLIENT_SECRET environment variable not found")?;

    tracing::debug!("Fetching new strava token using refresh token");

    let client = reqwest::Client::new();
    let resp = client
        .post("https://www.strava.com/oauth/token")
        .query(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token".to_string()),
        ])
        .send()
        .await
        .context("Failed to refresh Strava token")?;

    if resp.status().is_success() {
        let strava_data = resp.text().await;
        let strava_data: StravaTokenResponse =
            serde_json::from_str(&strava_data.unwrap()).context("Failed to deserialize JSON")?;

        let strava_data: TokenData = strava_data.into();

        db_service::set_strava_auth(strava_data.clone()).await;

        Ok(strava_data)
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}
