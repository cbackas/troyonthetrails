use std::collections::HashMap;

use shared_lib::{strava_structs::Activity, trail_structs::TrailSystem, utils};

pub mod ride_counts;
pub mod trail_data;

pub fn get_trail_ride_count(trails: Vec<TrailSystem>, rides: Vec<Activity>) -> HashMap<u64, i32> {
    let counts = rides.iter().fold(HashMap::new(), |mut counts, ride| {
        let closest_trail = trails
            .iter()
            .filter_map(|trail| {
                let distance = utils::haversine_distance(ride.clone(), trail.clone()).ok()?;
                (distance <= 3000.0).then_some((trail.id, distance))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((id, _)) = closest_trail {
            *counts.entry(id).or_insert(0) += 1;
        }

        counts
    });

    counts
}
