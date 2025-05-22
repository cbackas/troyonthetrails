use std::collections::HashMap;

use shared_lib::utils;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn setup() {
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(LevelFilter::INFO)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::test]
async fn external_test() {
    setup();
    let _db = db_service::get_db_service().await;

    tracing::info!("db ready, fetching rides");

    let rides = strava_service::get_all_activities()
        .await
        .expect("Failed to get activities");

    tracing::info!("Fetched rides, fetching trails");

    let trails = web_service::trail_lib::get_trail_data()
        .await
        .expect("Failed to get trail data");

    tracing::info!("Rides: {:?}, Trails: {:?}", rides.len(), trails.len());

    let counts = web_service::trail_lib::get_trail_ride_count(trails.clone(), rides.clone());
    let mut sorted_counts: Vec<_> = counts.iter().collect();
    sorted_counts.sort_by(|a, b| b.1.cmp(a.1));

    for (id, count) in sorted_counts.iter().take(10) {
        let trail = trails.iter().find(|t| t.id == **id).unwrap();
        tracing::info!("Trail: {}, Count: {}", trail.name, count);
    }
}
