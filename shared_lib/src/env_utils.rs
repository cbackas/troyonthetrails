use std::env;

use tracing::error;

use crate::utils::hash_string;

pub fn get_host_uri() -> String {
    match env::var("HOST") {
        Ok(host) => format!("https://{host}"),
        _ => match env::var("FLY_APP_NAME") {
            Ok(host) => format!("https://{host}.fly.dev"),
            _ => {
                format!("http://localhost:{}", get_port())
            }
        },
    }
}

pub fn get_port() -> u16 {
    let default_port: u16 = 8080;

    let port = match env::var("PORT") {
        Ok(port) => port,
        _ => default_port.to_string(),
    };
    let port: u16 = match port.parse::<_>() {
        Ok(port) => port,
        _ => {
            error!("Failed to parse PORT env var, using default");
            default_port
        }
    };

    port
}

pub fn get_webhook_secret() -> String {
    let wh_seed = match env::var("WH_SEED") {
        Ok(wh_seed) => wh_seed,
        _ => "defaultwebhookseed".to_string(),
    };

    hash_string(&wh_seed)[0..32].to_string()
}

pub fn get_strava_user_id() -> Option<String> {
    env::var("STRAVA_USER_ID").ok()
}

pub fn get_db_encryption_key() -> String {
    match env::var("DB_ENCRYPTION_KEY") {
        Ok(key) => key,
        _ => "defaultdbencryptionkey".to_string(),
    }
}

pub fn get_thunderforest_api_key() -> Option<String> {
    env::var("THUNDERFOREST_API_KEY").ok()
}
