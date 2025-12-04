use chrono::{DateTime, Local, TimeZone};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn format_time_friendly<T: TimeZone>(time: &DateTime<T>) -> String {
    let now = Local::now();
    let time_local = time.with_timezone(&Local);
    let duration = now.signed_duration_since(time_local);

    if duration.num_days() > 0 {
        time_local.format("%m/%d").to_string()
    } else if duration.num_hours() > 0 {
        time_local.format("%H:%M").to_string()
    } else {
        time_local.format("%H:%M").to_string()
    }
}

pub fn format_time_hhmm<T: TimeZone>(time: &DateTime<T>) -> String {
    time.with_timezone(&Local).format("%H:%M").to_string()
}

pub fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
}
