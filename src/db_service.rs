use std::{
    fmt::{Display, Formatter},
    time::{Duration, SystemTime},
};

use anyhow::Context;
use lazy_static::lazy_static;
use libsql_client::{args, Statement, SyncClient};
use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::strava_api_service::TokenData;

#[derive(Debug)]
pub struct TroyStatus {
    pub is_on_trail: bool,
    pub trail_status_updated: Option<SystemTime>,
}

enum DBTable {
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

lazy_static! {
    pub static ref DB_SERVICE: Mutex<DbService> = Mutex::new(DbService::new());
}

pub struct DbService {
    client: SyncClient,
}

impl Default for DbService {
    fn default() -> Self {
        Self::new()
    }
}

impl DbService {
    pub fn new() -> Self {
        let client = SyncClient::from_env()
            .context("Failed to connect to libsql db")
            .unwrap();

        DbService { client }
    }

    pub fn init_tables(&self) {
        match self.client.execute("CREATE TABLE IF NOT EXISTS troy_status (id INTEGER PRIMARY KEY CHECK (id = 1), is_on_trail INTEGER, trail_status_updated INTEGER)") {
            Err(err) => {
                error!("Failed to create table troy_status: {}", err);
            }
            Ok(_) => ()
        }

        match self.client
            .execute("CREATE TABLE IF NOT EXISTS strava_auth (id INTEGER PRIMARY KEY CHECK (id = 1), access_token TEXT, refresh_token TEXT, expires_at INTEGER)") {
                Err(err) => {
                    error!("Failed to create table strava_auth: {}", err);
                }
                Ok(_) => ()
            }
    }

    fn upsert(&self, statement: Statement, table: DBTable) {
        let result = self.client.execute(statement);

        if result.is_err() {
            error!("{}", result.unwrap_err());
            return;
        }

        let result = result.unwrap();
        if result.rows_affected != 1 {
            error!(
                "Failed to to db, expected 1 row affected but got {}",
                result.rows_affected
            );
            return;
        }

        debug!("{} upserted to db", table);
    }

    pub fn get_troy_status(&self) -> TroyStatus {
        let result = self.client.execute("SELECT * FROM troy_status");

        if result.is_err() {
            error!("Failed to get troy status from db");
            return TroyStatus {
                is_on_trail: false,
                trail_status_updated: None,
            };
        }

        let result = result.unwrap();

        if result.rows.len() != 1 {
            error!(
                "Failed to get troy status from db, expected 1 row but got {}",
                result.rows.len()
            );
            return TroyStatus {
                is_on_trail: false,
                trail_status_updated: None,
            };
        }

        let mut result = result.rows;
        let result = result.pop().unwrap();

        TroyStatus {
            is_on_trail: result
                .try_column("is_on_trail")
                .context("Failed to parse is_on_trail to int")
                .unwrap_or(0 as i64)
                == 1,
            trail_status_updated: Some(
                SystemTime::UNIX_EPOCH
                    + Duration::from_secs(
                        result
                            .try_column("trail_status_updated")
                            .unwrap_or(0 as i64) as u64,
                    ),
            ),
        }
    }

    pub fn set_troy_status(&self, is_on_trail: bool) {
        let is_on_trail = match is_on_trail {
            true => 1,
            false => 0,
        };

        // get current unix milis timestamp
        let current_timestamp: i64 = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
        {
            Ok(duration) => duration.as_secs() as i64,
            Err(_) => 0,
        };

        self
            .upsert(Statement::with_args(
                "INSERT INTO troy_status (id, is_on_trail, trail_status_updated) \
                VALUES (1, ?, ?) \
                ON CONFLICT (id) \
                DO UPDATE SET is_on_trail = excluded.is_on_trail, trail_status_updated = excluded.trail_status_updated",
                    args!(is_on_trail, current_timestamp),
            ), DBTable::TroyStatus);
    }

    pub fn get_strava_auth(&self) -> Option<TokenData> {
        let result = self.client.execute("SELECT * FROM strava_auth");
        if result.is_err() {
            error!("Failed to get strava auth from db");
            return None;
        }

        let result = result.unwrap();
        if result.rows.len() != 1 {
            error!(
                "Failed to get strava auth from db, expected 1 row but got {}",
                result.rows.len()
            );
            return None;
        }

        let mut result = result.rows;
        let result = result.pop().unwrap();

        Some(TokenData {
            access_token: result.try_column("access_token").unwrap_or("").to_string(),
            refresh_token: result.try_column("refresh_token").unwrap_or("").to_string(),
            expires_at: result.try_column("expires_at").unwrap_or(0 as u64) as u64,
        })
    }

    pub fn set_strava_auth(&self, token_data: TokenData) {
        let access_token = token_data.access_token;
        let refresh_token = token_data.refresh_token;
        let expires_at = token_data.expires_at as i64;

        self.upsert(Statement::with_args(
            "INSERT INTO strava_auth (id, access_token, refresh_token, expires_at) \
            VALUES (1, ?, ?, ?) \
            ON CONFLICT (id) \
            DO UPDATE SET access_token = excluded.access_token, refresh_token = excluded.refresh_token, expires_at = excluded.expires_at",
            args!(access_token, refresh_token, expires_at),
        ), DBTable::StravaAuth);
    }
}
