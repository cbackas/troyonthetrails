use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use tracing::debug;
use tracing::error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenData {
    pub expires_at: u64,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Athlete {
    pub id: u64,
    #[serde(flatten)]
    other: serde_json::Value, // This will capture all other fields within 'athlete' as a JSON value.
}

#[derive(Deserialize, Debug, Clone)]
pub struct StravaTokenResponse {
    pub token_type: String,
    pub expires_at: u64,
    pub expires_in: u64,
    pub refresh_token: String,
    pub access_token: String,
    pub athlete: Option<Athlete>,
}

fn file_path() -> PathBuf {
    let base_path = env::var("TOKEN_DATA_PATH").unwrap_or_else(|_| "/data".to_string());
    PathBuf::from(base_path).join(".strava_auth.json")
}

pub async fn read_token_data_from_file() -> io::Result<TokenData> {
    let path = file_path();
    let file_content = fs::read_to_string(path)?;
    let token_data = serde_json::from_str(&file_content)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    Ok(token_data)
}

pub async fn write_token_data_to_file(token_data: &TokenData) -> io::Result<()> {
    let path = file_path();
    let json_data = serde_json::to_string_pretty(token_data)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    fs::write(path, json_data)?;
    debug!("Wrote token data to file");
    Ok(())
}

fn strava_data_to_token_data(strava_data: StravaTokenResponse) -> TokenData {
    TokenData {
        expires_at: strava_data.expires_at,
        access_token: strava_data.access_token,
        refresh_token: strava_data.refresh_token,
    }
}

pub async fn get_token_from_code(code: String) -> anyhow::Result<TokenData> {
    let client_id = std::env::var("STRAVA_CLIENT_ID")
        .context("STRAVA_CLIENT_ID environment variable not found")?;
    let client_secret = std::env::var("STRAVA_CLIENT_SECRET")
        .context("STRAVA_CLIENT_SECRET environment variable not found")?;

    debug!("Fetching new strava token using OAuth flow");

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
            let strava_user_id = std::env::var("STRAVA_USER_ID")
                .context("STRAVA_USER_ID environment variable not found")?;
            if athlete.id.to_string() != strava_user_id {
                return Err(anyhow::anyhow!(
                    "Successfully authenticated Strava user but the user id does not match the defined STRAVA_USER_ID"
                ));
            }
        }

        let strava_data = strava_data_to_token_data(strava_data);

        let write_file = write_token_data_to_file(&strava_data).await;
        if write_file.is_err() {
            error!(
                "Failed to write token data to file: {}",
                write_file.unwrap_err()
            );
        }

        Ok(strava_data)
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}

pub async fn get_token_from_refresh(refresh_token: String) -> anyhow::Result<TokenData> {
    let client_id = std::env::var("STRAVA_CLIENT_ID")
        .context("STRAVA_CLIENT_ID environment variable not found")?;
    let client_secret = std::env::var("STRAVA_CLIENT_SECRET")
        .context("STRAVA_CLIENT_SECRET environment variable not found")?;

    debug!("Fetching new strava token using refresh token");

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
        let strava_data = strava_data_to_token_data(strava_data);

        let write_file = write_token_data_to_file(&strava_data).await;
        if write_file.is_err() {
            error!(
                "Failed to write token data to file: {}",
                write_file.unwrap_err()
            );
        }

        Ok(strava_data)
    } else {
        Err(anyhow::anyhow!(
            "Received a non-success status code {}: {}",
            resp.status(),
            resp.text().await.unwrap_or("Unknown error".to_string())
        ))
    }
}
