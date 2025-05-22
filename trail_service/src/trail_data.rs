use anyhow::Context;
use shared_lib::trail_structs::TrailSystem;
use tokio::{sync::OnceCell, time::Instant};

#[derive(Default, Clone)]
pub struct TrailDataCache {
    pub trail_data: Vec<TrailSystem>,
    pub last_updated: Option<Instant>,
}

static TRAIL_CACHE: OnceCell<TrailDataCache> = OnceCell::const_new();
pub async fn get_data() -> TrailDataCache {
    let trail_data_cache = TRAIL_CACHE
        .get_or_init(|| async {
            TrailDataCache {
                trail_data: fetch_trail_data().await.unwrap_or_default(),
                last_updated: Some(Instant::now()),
            }
        })
        .await;

    if let Some(last_updated) = trail_data_cache.last_updated {
        // if the trail data was updated less than 5 minutes ago, just use that
        if last_updated.elapsed().as_secs() < 300 {
            tracing::trace!("Using cached trail data");
            return trail_data_cache.clone();
        }
    }

    let trail_data = fetch_trail_data().await.unwrap_or_default();
    let is_empty = trail_data.is_empty();
    let mut cache = trail_data_cache.clone();
    cache.trail_data = trail_data;
    if is_empty {
        cache.last_updated = None;
    } else {
        cache.last_updated = Some(Instant::now());
    }
    cache
}

async fn fetch_trail_data() -> anyhow::Result<Vec<TrailSystem>> {
    let html = get_trail_html().await?;
    let trail_systems = extract_trail_data(html)?;
    let sorted_trail_systems = sort_trail_data(trail_systems);
    Ok(sorted_trail_systems)
}

async fn get_trail_html() -> anyhow::Result<String> {
    let url =
        std::env::var("TRAIL_DATA_URL").context("TRAIL_DATA_URL environment variable not found")?;

    let resp = reqwest::get(url)
        .await
        .context("Failed to get HTML from data source")?;
    let html = resp.text().await.context("Couldn't find html body")?;

    tracing::trace!("Fetched trail data from data source");

    Ok(html)
}

fn extract_trail_data(html: String) -> anyhow::Result<Vec<TrailSystem>> {
    let start_tag = "var trail_systems = ";
    let end_tag = ";</script>";

    let start = html
        .find(start_tag)
        .ok_or(anyhow::anyhow!("Start tag not found"))?
        + start_tag.len();
    let end = html[start..]
        .find(end_tag)
        .ok_or(anyhow::anyhow!("End tag not found"))?
        + start;

    let json = &html[start..end];

    let trail_systems = serde_json::from_str(json);
    match trail_systems {
        Ok(trail_systems) => Ok(trail_systems),
        Err(err) => Err(err.into()),
    }
}

fn sort_trail_data(trail_data: Vec<TrailSystem>) -> Vec<TrailSystem> {
    let static_lat = match std::env::var("HOME_LAT")
        .map_err(|e| e.to_string())
        .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
    {
        Ok(lat) => lat,
        Err(err) => {
            tracing::error!("Failed to parse HOME_LAT environment variable: {}", err);
            return trail_data;
        }
    };

    let static_lng = match std::env::var("HOME_LNG")
        .map_err(|e| e.to_string())
        .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
    {
        Ok(lng) => lng,
        Err(err) => {
            tracing::error!("Failed to parse HOME_LNG environment variable: {}", err);
            return trail_data;
        }
    };

    let mut sorted_data = trail_data;
    sorted_data.sort_by(|a, b| {
        let distance_a = ((a.lat - static_lat).powi(2) + (a.lng - static_lng).powi(2)).sqrt();
        let distance_b = ((b.lat - static_lat).powi(2) + (b.lng - static_lng).powi(2)).sqrt();
        distance_a
            .partial_cmp(&distance_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    sorted_data
}
