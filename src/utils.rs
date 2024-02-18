use sha2::{Digest, Sha256};

pub fn meters_to_feet(meters: f64, round_to_whole: bool) -> f64 {
    let feet = meters * 3.28084;
    if round_to_whole {
        feet.round()
    } else {
        (feet * 10.0).round() / 10.0
    }
}

pub fn meters_to_miles(meters: f64, round_to_whole: bool) -> f64 {
    let miles = meters * 0.000621371;
    if round_to_whole {
        miles.round()
    } else {
        (miles * 10.0).round() / 10.0
    }
}

pub fn mps_to_miph(mps: f64, round_to_whole: bool) -> f64 {
    let miph = mps * 2.23694;
    if round_to_whole {
        miph.round()
    } else {
        (miph * 10.0).round() / 10.0
    }
}

pub fn format_thousands(num: f64) -> String {
    let binding = num.to_string();
    let parts: Vec<&str> = binding.split('.').collect();
    let mut chars: Vec<char> = parts[0].chars().collect();
    let mut index = chars.len() as isize - 3;
    while index > 0 {
        chars.insert(index as usize, ',');
        index -= 3;
    }
    let integer_part: String = chars.into_iter().collect();
    if parts.len() > 1 {
        format!("{}.{}", integer_part, parts[1])
    } else {
        integer_part
    }
}

pub fn duration_to_ms_string(duration: std::time::Duration) -> String {
    // Convert the duration to milliseconds as f64
    let milliseconds =
        duration.as_secs_f64() * 1000.0 + duration.subsec_nanos() as f64 / 1_000_000.0;
    // Format the milliseconds to a string with 2 decimal places and add 'ms' postfix
    format!("{:.2}ms", milliseconds)
}

pub fn hash_string(string: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(string);
    format!("{:x}", hasher.finalize())
}
