use std::time::{Duration, Instant, SystemTime};

use anyhow::Context;
use lazy_static::lazy_static;
use libsql_client::{args, Statement, SyncClient};
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Debug)]
pub struct TroyStatus {
    pub is_on_trail: bool,
    pub trail_status_updated: Option<SystemTime>,
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

    // pub fn get_client(&self) -> Arc<SyncClient> {
    //     self.client.clone()
    // }

    pub fn init_tables(&self) {
        if !self.table_exists("troy_status") {
            info!("Creating table troy_status");
            let create_result= self.client
                .execute("CREATE TABLE troy_status (id INTEGER PRIMARY KEY CHECK (id = 1), is_on_trail INTEGER, trail_status_updated INTEGER)");
            if create_result.is_err() {
                error!("Failed to create table troy_status");
                return;
            }

            let _ = self.client.execute(
                "INSERT INTO troy_status (id, is_on_trail, trail_status_updated) VALUES (1, 0, 0)",
            );
        }

        if !self.table_exists("strava_auth") {
            info!("Creating table strava_auth");
            let _ = self.client
                .execute("CREATE TABLE strava_auth (id INTEGER PRIMARY KEY CHECK (id = 1), access_token TEXT, refresh_token TEXT, expires_at INTEGER)");
        }
    }

    fn table_exists(&self, table_name: &str) -> bool {
        let result = self
            .client
            .execute(Statement::with_args(
                "SELECT name FROM sqlite_master WHERE type='table' AND name= ?",
                args!(table_name),
            ))
            .context("Failed to check for expected tables in db")
            .unwrap();

        result.rows.len() > 0
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

        let is_on_trail = result
            .try_column("is_on_trail")
            .context("Failed to parse is_on_trail to int")
            .unwrap_or(0 as i64)
            == 1;
        let trail_status_updated = result
            .try_column("trail_status_updated")
            .unwrap_or(0 as i64) as u64;
        let trail_status_updated =
            SystemTime::UNIX_EPOCH + Duration::from_secs(trail_status_updated);
        let trail_status_updated = Some(trail_status_updated);

        TroyStatus {
            is_on_trail,
            trail_status_updated,
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

        let _ = self
            .client
            .execute(Statement::with_args(
                "UPDATE troy_status SET is_on_trail = ?, trail_status_updated = ? WHERE id = 1",
                args!(is_on_trail, current_timestamp),
            ))
            .context("Failed to set troy status in db")
            .unwrap();
    }
}
