use crate::discord;
use strava_service::beacon::{BeaconData, Status};

// loop that continuously checks the db for a beacon url and processes the data if found
pub fn start() {
    match (std::env::var("FLY_REGION"), std::env::var("PRIMARY_REGION")) {
        (Ok(fly_region), Ok(primary_region)) => {
            if fly_region == primary_region {
                tracing::info!("Beacon loop running in region: {}", fly_region);
            } else {
                tracing::trace!(
                    "Fly region ({}) and primary region ({}) do not match, skipping beacon loop",
                    fly_region,
                    primary_region
                );
                return;
            }
        }
        _ => {
            tracing::warn!("FLY_REGION and PRIMARY_REGION are not both set, running beacon loop");
        }
    }

    tokio::spawn(async move {
        loop {
            process_beacon().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(45)).await;
        }
    });
}

async fn process_beacon() {
    let troy_status = db_service::get_troy_status().await;

    let beacon_url = match troy_status.beacon_url {
        Some(url) => url,
        None => {
            if troy_status.is_on_trail {
                tracing::warn!(
                "Troy status indicates on the trails but no beacon url found, clearing troy status"
            );
                db_service::set_troy_status(false).await;
            } else {
                tracing::debug!("No beacon url found, troy is not on the trails");
            }
            return;
        }
    };

    let beacon_data = match strava_service::beacon::get_beacon_data(beacon_url.to_string()).await {
        Ok(data) => data,
        Err(e) if e.to_string().contains("404 Not Found") => {
            tracing::warn!("Beacon data not found (404 Not Found), clearing beacon url");
            db_service::set_beacon_url(None).await;
            return;
        }
        Err(e) => {
            tracing::error!("Failed to get beacon data: {}", e);
            return;
        }
    };

    let BeaconData {
        status,
        activity_id,
        update_time,
        ..
    } = match (beacon_data.activity_id, &beacon_data.status) {
        // has activity_id, status is already uploaded or discarded
        (Some(_), Status::Uploaded | Status::Discarded) => beacon_data,
        // has activity_id, but status is neither uploaded nor discarded
        (Some(_), _) => {
            let mut beacon_data = beacon_data;
            beacon_data.status = Status::Uploaded;
            beacon_data
        }
        // no activity_id, but Uploaded status (which is a lie)
        (None, Status::Uploaded) => {
            let mut beacon_data = beacon_data;
            beacon_data.status = Status::UploadedLie;
            beacon_data
        }
        // no activity_id, and status is anything else
        _ => beacon_data,
    };

    let ride_time = {
        let update_time = update_time.datetime();
        let now = chrono::Utc::now();
        (now - update_time).num_minutes()
    };

    match status {
        Status::Active | Status::AutoPaused | Status::ManualPaused => {
            tracing::trace!("Beacon data indicates troy is active on the trails");
            db_service::set_troy_status(true).await;
            if !troy_status.is_on_trail {
                tracing::info!("Troy status updated to on the trails");
                discord::send_starting_webhook(beacon_url).await;
            }
        }
        Status::Uploaded => {
            tracing::info!("Beacon data indicates activity uploaded, clearing beacon url");
            db_service::set_beacon_url(None).await;
            if troy_status.is_on_trail {
                db_service::set_troy_status(false).await;
                discord::send_end_webhook(activity_id).await;
            }
        }
        Status::Discarded => {
            tracing::info!(
                "Beacon data indicates activity was discarded, clearing troy status and beacon url"
            );
            db_service::set_beacon_url(None).await;
            if troy_status.is_on_trail {
                db_service::set_troy_status(false).await;
                discord::send_discard_webhook().await;
            }
        }
        Status::NotStarted => {
            tracing::info!("Beacon data indicates activity is not started yet");
            if ride_time > 45 {
                tracing::info!(
                    "Beacon data is old and activity never started, clearing beacon url"
                );
                db_service::set_beacon_url(None).await;
            }
        }
        Status::UploadedLie => {
            if ride_time > (4 * 60) {
                tracing::info!("Beacon data indicates activity was uploaded, but no activity id was found. It's been a while, clearing beacon url");
                db_service::set_troy_status(false).await;
                discord::send_end_webhook(None).await;
            } else {
                tracing::info!("Beacon data indicates activity was uploaded, but no activity id found, looping back again");
            }
        }
        _ => {
            tracing::warn!("Beacon data indicates unknown status");
        }
    }
}
