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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_format_time_hhmm() {
        // Create a specific time in UTC: 2023-01-01T15:30:00Z
        let time = Utc.with_ymd_and_hms(2023, 1, 1, 15, 30, 0).unwrap();
        let formatted = format_time_hhmm(&time);

        // The output depends on the local timezone, so we format the same time
        // using the local timezone to get the expected output.
        let expected = time.with_timezone(&Local).format("%H:%M").to_string();

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_now_millis() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let now = now_millis();

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        assert!(now >= before);
        assert!(now <= after);
    }
}
