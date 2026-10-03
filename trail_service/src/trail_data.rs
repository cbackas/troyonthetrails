use anyhow::Context;
use shared_lib::trail_structs::TrailSystem;
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use tokio::time::Instant;

#[derive(Default, Clone)]
pub struct TrailDataCache {
    pub trail_data: Vec<TrailSystem>,
    pub last_updated: Option<Instant>,
}

static TRAIL_CACHE: LazyLock<Arc<Mutex<Option<TrailDataCache>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));
pub async fn get_data() -> TrailDataCache {
    let mut guard = TRAIL_CACHE.lock().await;

    if let Some(data) = guard.as_ref() {
        if data
            .last_updated
            .is_some_and(|t| t.elapsed().as_secs() < 300)
        {
            tracing::trace!("Using cached trail data");
            return data.clone();
        }
        tracing::trace!("Trail data is stale, fetching new data");
    } else {
        tracing::trace!("Fetching trail data for the first time");
    }

    let fetched = fetch_trail_data().await.unwrap_or_default();

    // Reset the staleness timer on every attempt so a struggling upstream is only
    // hit once per window, but never let an empty fetch clobber good data.
    let trail_data = if fetched.is_empty() {
        match guard.as_ref().map(|d| &d.trail_data) {
            Some(cached) if !cached.is_empty() => {
                tracing::warn!("Trail data fetch was empty, keeping last good data");
                cached.clone()
            }
            _ => {
                tracing::warn!("Trail data fetch was empty, no good data to serve");
                Vec::new()
            }
        }
    } else {
        fetched
    };

    let updated = TrailDataCache {
        trail_data,
        last_updated: Some(Instant::now()),
    };
    *guard = Some(updated.clone());
    updated
}

struct TrailCollection(Vec<TrailSystem>);

impl TrailCollection {
    fn sort_by_distance(mut self) -> Self {
        let static_lat = match std::env::var("HOME_LAT")
            .map_err(|e| e.to_string())
            .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
        {
            Ok(lat) => lat,
            Err(err) => {
                tracing::error!("Failed to parse HOME_LAT environment variable: {}", err);
                return self;
            }
        };

        let static_lng = match std::env::var("HOME_LNG")
            .map_err(|e| e.to_string())
            .and_then(|s| s.parse::<f64>().map_err(|e| e.to_string()))
        {
            Ok(lng) => lng,
            Err(err) => {
                tracing::error!("Failed to parse HOME_LNG environment variable: {}", err);
                return self;
            }
        };

        self.0.sort_by(|a, b| {
            let distance_a = ((a.lat - static_lat).powi(2) + (a.lng - static_lng).powi(2)).sqrt();
            let distance_b = ((b.lat - static_lat).powi(2) + (b.lng - static_lng).powi(2)).sqrt();
            distance_a
                .partial_cmp(&distance_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        self
    }

    fn filter_redundant_child_trails(mut self) -> Self {
        // Drop a skills course when a nearby park shares its 2+ word name prefix.
        const SKILLS_COURSE: &str = "skills course";
        const MIN_SHARED_WORDS: usize = 2;

        let is_skills_course = |name: &str| name.to_lowercase().contains(SKILLS_COURSE);

        // Precompute the potential parents (everything that isn't a skills course).
        let parents: Vec<(Vec<String>, f64, f64)> = self
            .0
            .iter()
            .filter(|t| !is_skills_course(&t.name))
            .map(|t| {
                (
                    t.name.split_whitespace().map(str::to_lowercase).collect(),
                    t.lat,
                    t.lng,
                )
            })
            .collect();

        self.0.retain(|trail| {
            if !is_skills_course(&trail.name) {
                return true;
            }
            let words: Vec<String> = trail
                .name
                .split_whitespace()
                .map(str::to_lowercase)
                .collect();

            // Keep it unless a nearby parent shares a 2+ word prefix.
            !parents
                .iter()
                .any(|(parent_words, parent_lat, parent_lng)| {
                    let shared = words
                        .iter()
                        .zip(parent_words)
                        .take_while(|(a, b)| a == b)
                        .count();
                    if shared < MIN_SHARED_WORDS {
                        return false;
                    }
                    // Within ~2km, using an approximate degree-to-km conversion.
                    let dlat = (trail.lat - parent_lat) * 111.0;
                    let dlng = (trail.lng - parent_lng) * 85.0;
                    dlat * dlat + dlng * dlng < 4.0
                })
        });
        self
    }

    fn into_inner(self) -> Vec<TrailSystem> {
        self.0
    }
}

async fn fetch_trail_data() -> anyhow::Result<Vec<TrailSystem>> {
    let html = get_trail_html().await?;
    let trails = extract_trail_data(html)?
        .sort_by_distance()
        .filter_redundant_child_trails()
        .into_inner();
    Ok(trails)
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

fn extract_trail_data(html: String) -> anyhow::Result<TrailCollection> {
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

    let trail_systems: Vec<TrailSystem> = serde_json::from_str(json)?;
    Ok(TrailCollection(trail_systems))
}
