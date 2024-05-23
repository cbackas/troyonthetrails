use crate::{
    db_service, discord,
    strava::{self, beacon::Status},
};

pub async fn process_beacon() {
    let troy_status = db_service::get_troy_status().await;

    if troy_status.beacon_url.is_none() && troy_status.is_on_trail {
        tracing::warn!(
            "Troy status indicates on the trails but no beacon url found, clearing troy status"
        );
        db_service::set_troy_status(false).await;
        return;
    }

    let beacon_data = match troy_status.beacon_url {
        Some(beacon_url) => match strava::beacon::get_beacon_data(beacon_url).await {
            Ok(data) => Some(data),
            Err(e) => {
                tracing::error!("Failed to get beacon data: {}", e);
                None
            }
        },
        None => None,
    };

    let (activity_status, activity_id) = match beacon_data.clone() {
        Some(data) => (Some(data.status), data.activity_id),
        None => (None, None),
    };

    match activity_status {
        Some(Status::Active | Status::AutoPaused | Status::ManualPaused) => {
            tracing::trace!("Beacon data indicates troy is active on the trails");
            db_service::set_troy_status(true).await;
            if !troy_status.is_on_trail {
                tracing::info!("Troy status updated to on the trails");
                discord::send_starting_webhook().await;
            }
        }
        Some(Status::Uploaded) => {
            tracing::info!("Beacon data indicates activity uploaded, clearing beacon url");
            db_service::set_beacon_url(None).await;
            if troy_status.is_on_trail {
                db_service::set_troy_status(false).await;
                discord::send_end_webhook(activity_id).await;
            }
        }
        Some(Status::Dicarded) => {
            tracing::info!(
                "Beacon data indicates activity was discarded, clearing troy status and beacon url"
            );
            db_service::set_beacon_url(None).await;
            if troy_status.is_on_trail {
                db_service::set_troy_status(false).await;
                discord::send_discard_webhook().await;
            }
        }
        Some(Status::NotStarted) => {
            tracing::info!("Beacon data indicates activity is not started yet");
            let diff = {
                let update_time = beacon_data.unwrap().update_time;
                let update_time = update_time.datetime();
                let now = chrono::Utc::now();
                now - update_time
            };
            if diff.num_minutes() > 45 {
                tracing::info!(
                    "Beacon data is old and activity never started, clearing beacon url"
                );
                db_service::set_beacon_url(None).await;
            }
        }
        None => {}
        _ => {
            tracing::warn!("Beacon data indicates unknown status");
        }
    }
}
