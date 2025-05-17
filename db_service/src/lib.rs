mod encryption;

use std::{
    env,
    fmt::{Debug, Display, Formatter},
    time::{Duration, SystemTime},
};

use anyhow::Context;
use libsql::{de::from_row, params::IntoParams};
use serde::de;
use tokio::sync::OnceCell;

use crate::encryption::{decrypt, encrypt};
use shared_lib::structs::TokenData;

static DB_SERVICE: OnceCell<DbService> = OnceCell::const_new();

#[derive(Debug)]
pub struct TroyStatus {
    pub is_on_trail: bool,
    pub beacon_url: Option<String>,
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

pub async fn get_db_service() -> &'static DbService {
    DB_SERVICE
        .get_or_init(|| async {
            let db = libsql::Builder::new_remote_replica(
                env::var("LIBSQL_LOCAL_DB_PATH").unwrap_or("file:local_replica.db".to_string()),
                env::var("LIBSQL_CLIENT_URL").expect("Missing LIBSQL_CLIENT_URL"),
                env::var("LIBSQL_CLIENT_TOKEN").expect("Missing LIBSQL_CLIENT_TOKEN"),
            )
            .build()
            .await
            .expect("Failed to create database");

            tracing::debug!("Initialized db");

            let _ = db.sync().await.expect("Failed to sync db");

            tracing::trace!("Synced remote db to local disk");

            DbService { db }
        })
        .await
}

pub struct DbService {
    db: libsql::Database,
}

impl DbService {
    pub async fn init_tables(&self) {
        let conn = self.db.connect().expect("Failed to connect to db");
        let _ = conn
            .execute(
                "CREATE TABLE IF NOT EXISTS troy_status (id INTEGER PRIMARY KEY CHECK (id = 1), is_on_trail INTEGER, beacon_url TEXT, trail_status_updated INTEGER)",
                libsql::params!(),
            )
            .await;
        let _ = conn
            .execute(
                "CREATE TABLE IF NOT EXISTS strava_auth (id INTEGER PRIMARY KEY CHECK (id = 1), access_token TEXT, refresh_token TEXT, expires_at INTEGER)",
                libsql::params!(),
            )
            .await;

        let _ = conn
            .execute(
                "ALTER TABLE troy_status ADD COLUMN beacon_url TEXT",
                libsql::params!(),
            )
            .await;
    }

    // execute the statement and return the number of rows affected
    // also syncs the DB with remote primary
    pub async fn execute(
        &self,
        statement: &str,
        params: impl IntoParams,
        table: DBTable,
    ) -> anyhow::Result<u64> {
        let db = &self.db;
        let result = db.connect()?.execute(statement, params).await;

        if result.is_err() {
            return Err(result.err().unwrap().into());
        }

        let result = result.unwrap();
        if result == 0 {
            tracing::error!(
                "Failed to to db, expected 1 row affected but got {}",
                result
            );
            return Err(anyhow::anyhow!(
                "Failed to to db, expected 1 row affected but got {}",
                result
            ));
        }

        tracing::trace!("{} upserted to db", table);

        let _sync = db.sync().await?;
        Ok(result)
    }

    pub async fn query_many<T>(
        &self,
        statement: &str,
        params: impl IntoParams,
    ) -> anyhow::Result<Vec<T>>
    where
        T: de::DeserializeOwned,
        T: Debug,
        T: Clone,
    {
        let connection = &self.db.connect().expect("Failed to connect to db");
        let mut result_set = connection
            .query(statement, params)
            .await
            .context("Failed to get data from database")?;

        let mut rows = Vec::new();
        while let Some(row) = result_set.next().await? {
            let row = libsql::de::from_row::<T>(&row)?;
            rows.push(row);
        }

        Ok(rows)
    }

    pub async fn query_one<T>(&self, statement: &str, params: impl IntoParams) -> anyhow::Result<T>
    where
        T: de::DeserializeOwned,
        T: Debug,
        T: Clone,
    {
        let connection = &self.db.connect().expect("Failed to connect to db");
        let mut result_set = connection
            .query(statement, params)
            .await
            .context("Failed to get data from database")?;

        let row = result_set
            .next()
            .await
            .context("Failed to get data from database")?
            .context("Failed to get data from database")?;

        let row = from_row::<T>(&row)?;

        Ok(row)
    }
}

pub async fn get_troy_status() -> TroyStatus {
    #[derive(Debug, serde::Deserialize, Clone)]
    #[allow(dead_code)]
    struct TroyStatusRow {
        id: i64,
        is_on_trail: u8,
        beacon_url: Option<String>,
        trail_status_updated: u64,
    }

    let db_service = DB_SERVICE.get().unwrap();
    let result = db_service
        .query_one::<TroyStatusRow>("SELECT * FROM troy_status", libsql::params!())
        .await;

    match result {
        Ok(result) => TroyStatus {
            is_on_trail: result.is_on_trail == 1,
            beacon_url: result.beacon_url.clone(),
            trail_status_updated: Some(
                SystemTime::UNIX_EPOCH + Duration::from_secs(result.trail_status_updated),
            ),
        },
        Err(_) => TroyStatus {
            is_on_trail: false,
            beacon_url: None,
            trail_status_updated: None,
        },
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

    tracing::debug!("Updating troy status in the DB to {}", is_on_trail);

    let _ = DB_SERVICE.get().unwrap()
            .execute(
                "INSERT INTO troy_status (id, is_on_trail, trail_status_updated) \
                VALUES (1, ?, ?) \
                ON CONFLICT (id) \
                DO UPDATE SET is_on_trail = excluded.is_on_trail, trail_status_updated = excluded.trail_status_updated",
                libsql::params!(is_on_trail, current_timestamp),
                DBTable::TroyStatus).await;
}

pub async fn set_beacon_url(beacon_url: Option<String>) {
    tracing::debug!("Updating beacon url in the DB to {:?}", beacon_url);
    let _ = DB_SERVICE
        .get()
        .unwrap()
        .execute(
            "INSERT INTO troy_status (id, beacon_url) \
                VALUES (1, ?) \
                ON CONFLICT (id) \
                DO UPDATE SET beacon_url = excluded.beacon_url",
            libsql::params!(beacon_url),
            DBTable::TroyStatus,
        )
        .await;
}

pub async fn get_strava_auth() -> anyhow::Result<TokenData> {
    #[derive(Debug, serde::Deserialize, Clone)]
    #[allow(dead_code)]
    struct StravaAuthRow {
        id: i64,
        expires_at: u64,
        access_token: Vec<u8>,
        refresh_token: Vec<u8>,
    }

    let db_service = DB_SERVICE.get().unwrap();
    let result = db_service
        .query_one::<StravaAuthRow>("SELECT * FROM strava_auth", libsql::params!())
        .await;

    match result {
        Ok(result) => Ok(TokenData {
            expires_at: result.expires_at,
            access_token: decrypt(result.access_token)?,
            refresh_token: decrypt(result.refresh_token)?,
        }),
        Err(e) => Err(e),
    }
}

pub async fn set_strava_auth(token_data: TokenData) {
    let access_token = match encrypt(token_data.access_token) {
        Ok(token) => token,
        Err(error) => {
            tracing::error!("Failed to encrypt access token {:?}", error);
            return;
        }
    };

    let refresh_token = match encrypt(token_data.refresh_token) {
        Ok(token) => token,
        Err(error) => {
            tracing::error!("Failed to encrypt refresh token {:?}", error);
            return;
        }
    };

    tracing::debug!("Updating strava auth in the DB");

    let _ = DB_SERVICE.get().unwrap().execute(
            "INSERT INTO strava_auth (id, access_token, refresh_token, expires_at) \
            VALUES (1, ?, ?, ?) \
            ON CONFLICT (id) \
            DO UPDATE SET access_token = excluded.access_token, refresh_token = excluded.refresh_token, expires_at = excluded.expires_at",
            libsql::params!(access_token, refresh_token, token_data.expires_at),
        DBTable::StravaAuth).await;
}
