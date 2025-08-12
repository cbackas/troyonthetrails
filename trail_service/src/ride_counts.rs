use std::collections::HashMap;

use shared_lib::{
    strava_structs::Activity,
    trail_structs::{TrailStats, TrailSystem},
    utils,
};

pub fn calculate_stats(trails: Vec<TrailSystem>, rides: Vec<Activity>) -> HashMap<u64, TrailStats> {
    let counts = rides.iter().fold(HashMap::new(), |mut counts, ride| {
        let closest_trail = trails
            .iter()
            .filter_map(|trail| {
                let distance = utils::haversine_distance(ride.clone(), trail.clone()).ok()?;
                (distance <= 3000.0).then_some((trail.id, distance))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((id, _)) = closest_trail {
            let entry = counts.entry(id).or_insert(TrailStats {
                id,
                ..Default::default()
            });

            entry.rides += 1;
            entry.achievement_count += ride.achievement_count;
            entry.total_moving_time += ride.moving_time;
        }

        counts
    });

    counts
}
