use std::{
    env,
    fmt::{Debug, Display, Formatter},
    time::{Duration, SystemTime},
};

use libsql::params::IntoParams;
use tracing;

use crate::{
    encryption::{decrypt, encrypt},
    strava::auth::TokenData,
    DB_SERVICE,
};

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
            let db = libsql::Database::open_with_remote_sync(
                env::var("LIBSQL_LOCAL_DB_PATH").unwrap_or("file:local_replica.db".to_string()),
                env::var("LIBSQL_CLIENT_URL").unwrap(),
                env::var("LIBSQL_CLIENT_TOKEN").unwrap(),
            )
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

        tracing::debug!("{} upserted to db", table);

        let _sync = db.sync().await?;
        Ok(result)
    }
}

pub async fn get_troy_status() -> TroyStatus {
    let db_service = DB_SERVICE.get().unwrap();
    let result = db_service
        .db
        .connect()
        .expect("Failed to connect to db")
        .query("SELECT * FROM troy_status", libsql::params!())
        .await;

    if result.is_err() {
        tracing::error!("Failed to get troy status from db");
        return TroyStatus {
            is_on_trail: false,
            beacon_url: None,
            trail_status_updated: None,
        };
    }

    let result = match result.unwrap().next() {
        Err(_) => None,
        Ok(result) => result,
    };

    if result.is_none() {
        tracing::error!("Failed to get troy status from db, didn't find any rows",);
        return TroyStatus {
            is_on_trail: false,
            beacon_url: None,
            trail_status_updated: None,
        };
    }

    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct TroyStatusRow {
        id: i64,
        is_on_trail: u8,
        beacon_url: Option<String>,
        trail_status_updated: u64,
    }

    let result = result.unwrap();

    let thing = libsql::de::from_row::<TroyStatusRow>(&result).unwrap();

    TroyStatus {
        is_on_trail: thing.is_on_trail == 1,
        beacon_url: thing.beacon_url,
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

pub async fn get_strava_auth() -> Option<TokenData> {
    let result = DB_SERVICE
        .get()
        .unwrap()
        .db
        .connect()
        .expect("Failed to connect to db")
        .query("SELECT * FROM strava_auth", libsql::params!())
        .await;

    if result.is_err() {
        // let thing = result.unwrap_err().to_string();
        tracing::error!("Failed to get strava auth from db");
        return None;
    }

    let result = match result.unwrap().next() {
        Err(_) => None,
        Ok(result) => result,
    };

    if result.is_none() {
        tracing::error!("Failed to get strava auth from db, expected 1 row but found none");
        return None;
    }

    let result = result.unwrap();

    let access_token = result.get(1).unwrap_or("".into());
    let access_token = decrypt(access_token).expect("Failed to decrypt access token");

    let refresh_token = result.get(2).unwrap_or("".into());
    let refresh_token = decrypt(refresh_token).expect("Failed to decrypt refresh token");

    Some(TokenData {
        access_token,
        refresh_token,
        expires_at: result.get(3).unwrap_or(0),
    })
}

pub async fn set_strava_auth(token_data: TokenData) {
    let access_token = encrypt(token_data.access_token);
    if let Err(error) = access_token {
        tracing::error!("Failed to encrypt access token {:?}", error);
        return;
    }

    let refresh_token = encrypt(token_data.refresh_token);
    if let Err(error) = refresh_token {
        tracing::error!("Failed to encrypt refresh token {:?}", error);
        return;
    }

    let _ = DB_SERVICE.get().unwrap().execute(
            "INSERT INTO strava_auth (id, access_token, refresh_token, expires_at) \
            VALUES (1, ?, ?, ?) \
            ON CONFLICT (id) \
            DO UPDATE SET access_token = excluded.access_token, refresh_token = excluded.refresh_token, expires_at = excluded.expires_at",
            libsql::params!(access_token.unwrap(), refresh_token.unwrap(), token_data.expires_at),
        DBTable::StravaAuth).await;
}
