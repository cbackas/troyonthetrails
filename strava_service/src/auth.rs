use anyhow::Context;

use db_service;
use shared_lib::strava_structs::{StravaTokenResponse, TokenData};
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::Mutex;

static TOKEN_DATA: LazyLock<Arc<Mutex<Option<TokenData>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));
pub async fn get_token() -> Option<TokenData> {
    let mut guard = TOKEN_DATA.lock().await;

    if let Some(ref data) = *guard {
        if data.expires_at >= chrono::Utc::now().timestamp() as u64 {
            return Some(data.clone());
        }
        tracing::warn!("Strava token has expired");
        if let Ok(new_token) = get_token_from_refresh(data.refresh_token.clone()).await {
            *guard = Some(new_token.clone());
            return Some(new_token);
        } else {
            tracing::error!("Failed to refresh strava token");
            return None;
        }
    }

    match db_service::get_strava_auth().await {
        Ok(data) => {
            *guard = Some(data.clone());
            Some(data)
        }
        Err(e) => {
            tracing::warn!("No strava auth data found in db, {:?}", e);
            None
        }
    }
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

        {
            let mut guard = TOKEN_DATA.lock().await;
            *guard = Some(strava_data.clone());
        }

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
