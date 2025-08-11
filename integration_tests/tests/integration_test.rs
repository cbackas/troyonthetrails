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

    let _rides = strava_service::get_all_activities()
        .await
        .expect("Failed to get activities");

    tracing::info!("Fetched rides, fetching trails");
}
