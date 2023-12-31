use std::{
    env,
    fmt::{Display, Formatter},
    time::{Duration, SystemTime},
};

use anyhow::Context;
use libsql::{params::IntoParams, Connection};
use tracing::{debug, error, trace};

use crate::{strava_api_service::TokenData, DB_SERVICE};

#[derive(Debug)]
pub struct TroyStatus {
    pub is_on_trail: bool,
    pub trail_status_updated: Option<SystemTime>,
}

pub enum DBTable {
    TroyStatus,
    StravaAuth,
}

impl Display for DBTable {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            DBTable::TroyStatus => write!(f, "troy_status"),
            DBTable::StravaAuth => write!(f, "strava_auth"),
        }
    }
}

pub struct DbService {
    database: Connection,
}

impl Default for DbService {
    fn default() -> Self {
        Self::new()
    }
}

impl DbService {
    pub fn new() -> Self {
        trace!("initializing new DbService");

        let database = libsql::Database::open_remote(
            env::var("LIBSQL_CLIENT_URL").unwrap(),
            env::var("LIBSQL_CLIENT_TOKEN").unwrap(),
        )
        .unwrap()
        .connect()
        .context("Failed to create database")
        .unwrap();

        DbService { database }
    }

    pub async fn init_tables(&self) {
        if let Err(err) = self.database.execute("CREATE TABLE IF NOT EXISTS troy_status (id INTEGER PRIMARY KEY CHECK (id = 1), is_on_trail INTEGER, trail_status_updated INTEGER)", libsql::params!()).await {
            error!("Failed to create table troy_status: {}", err);
        }

        if let Err(err) = self.database.execute("CREATE TABLE IF NOT EXISTS strava_auth (id INTEGER PRIMARY KEY CHECK (id = 1), access_token TEXT, refresh_token TEXT, expires_at INTEGER)", libsql::params!()).await {
            error!("Failed to create table strava_auth: {}", err);
        }
    }

    pub async fn execute(&self, statement: &str, params: impl IntoParams, table: DBTable) {
        let result = self.database.execute(statement, params).await;

        if result.is_err() {
            error!("{}", result.unwrap_err());
            return;
        }

        let result = result.unwrap();
        if result != 1 {
            error!(
                "Failed to to db, expected 1 row affected but got {}",
                result
            );
            return;
        }

        debug!("{} upserted to db", table);
    }
}

pub async fn get_troy_status() -> TroyStatus {
    let result = DB_SERVICE
        .lock()
        .await
        .database
        .query("SELECT * FROM troy_status", libsql::params!())
        .await;

    if result.is_err() {
        error!("Failed to get troy status from db");
        return TroyStatus {
            is_on_trail: false,
            trail_status_updated: None,
        };
    }

    let result = match result.unwrap().next() {
        Err(_) => None,
        Ok(result) => result,
    };

    if result.is_none() {
        error!("Failed to get troy status from db, didn't find any rows",);
        return TroyStatus {
            is_on_trail: false,
            trail_status_updated: None,
        };
    }

    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct TroyStatusRow {
        id: i64,
        is_on_trail: u8,
        trail_status_updated: u64,
    }

    let result = result.unwrap();

    let thing = libsql::de::from_row::<TroyStatusRow>(&result).unwrap();

    TroyStatus {
        is_on_trail: thing.is_on_trail == 1,
        trail_status_updated: Some(
            SystemTime::UNIX_EPOCH + Duration::from_secs(thing.trail_status_updated),
        ),
    }
}

pub async fn set_troy_status(is_on_trail: bool) {
    let is_on_trail = match is_on_trail {
        true => 1,
        false => 0,
    };

    // get current unix milis timestamp
    let current_timestamp: i64 = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        Err(_) => 0,
    };

    DB_SERVICE.lock().await
            .execute(
                "INSERT INTO troy_status (id, is_on_trail, trail_status_updated) \
                VALUES (1, ?, ?) \
                ON CONFLICT (id) \
                DO UPDATE SET is_on_trail = excluded.is_on_trail, trail_status_updated = excluded.trail_status_updated",
                libsql::params!(is_on_trail, current_timestamp),
                DBTable::TroyStatus).await;
}

pub async fn get_strava_auth() -> Option<TokenData> {
    let result = DB_SERVICE
        .lock()
        .await
        .database
        .query("SELECT * FROM strava_auth", libsql::params!())
        .await;

    if result.is_err() {
        error!("Failed to get strava auth from db");
        return None;
    }

    let result = match result.unwrap().next() {
        Err(_) => None,
        Ok(result) => result,
    };

    if result.is_none() {
        error!("Failed to get strava auth from db, expected 1 row but found none");
        return None;
    }

    let result = result.unwrap();

    Some(TokenData {
        access_token: result.get(1).unwrap_or("".to_string()),
        refresh_token: result.get(2).unwrap_or("".to_string()),
        expires_at: result.get(3).unwrap_or(0),
    })
}

pub async fn set_strava_auth(token_data: TokenData) {
    let access_token = token_data.access_token;
    let refresh_token = token_data.refresh_token;
    let expires_at = token_data.expires_at as i64;

    DB_SERVICE.lock().await.execute(
            "INSERT INTO strava_auth (id, access_token, refresh_token, expires_at) \
            VALUES (1, ?, ?, ?) \
            ON CONFLICT (id) \
            DO UPDATE SET access_token = excluded.access_token, refresh_token = excluded.refresh_token, expires_at = excluded.expires_at",
            libsql::params!(access_token, refresh_token, expires_at),
        DBTable::StravaAuth).await;
}
