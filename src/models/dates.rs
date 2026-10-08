use chrono::{DateTime, LocalResult, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use chrono_tz::Tz;

pub fn zone(name: &str) -> Tz {
    name.parse().unwrap_or(chrono_tz::UTC)
}

pub fn local(utc: &str, timezone: &str) -> Option<DateTime<Tz>> {
    DateTime::parse_from_rfc3339(utc)
        .ok()
        .map(|date| date.with_timezone(&zone(timezone)))
}

pub fn display(utc: &str, timezone: &str) -> String {
    local(utc, timezone).map_or_else(
        || utc.to_owned(),
        |date| date.format("%B %d, %Y at %-I:%M %p").to_string(),
    )
}

pub fn days_until(utc: &str, now: DateTime<Utc>, timezone: &str) -> Option<i64> {
    local(utc, timezone).map(|date| {
        (date.date_naive() - now.with_timezone(&zone(timezone)).date_naive()).num_days()
    })
}

fn parse_input(value: &str) -> Result<NaiveDateTime, &'static str> {
    let bytes = value.as_bytes();
    if bytes.len() != 16
        || value.starts_with("0000")
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| ![4, 7, 10, 13].contains(&index) && !byte.is_ascii_digit())
    {
        return Err("Enter a valid date and time.");
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
        .map_err(|_| "Enter a valid date and time.")
}

pub fn repeated_offsets(value: &str, timezone: &str) -> Vec<String> {
    let Ok(date) = parse_input(value) else {
        return Vec::new();
    };
    match zone(timezone).from_local_datetime(&date) {
        LocalResult::Ambiguous(first, second) => vec![
            first.format("%:z").to_string(),
            second.format("%:z").to_string(),
        ],
        _ => Vec::new(),
    }
}

pub fn to_utc(value: &str, timezone: &str, offset: &str) -> Result<String, &'static str> {
    let date = parse_input(value)?;
    let date = match zone(timezone).from_local_datetime(&date) {
        LocalResult::Single(date) => date,
        LocalResult::None => {
            return Err(
                "This time does not exist because the clocks move forward. Choose another time.",
            );
        }
        LocalResult::Ambiguous(first, second) => {
            if first.format("%:z").to_string() == offset {
                first
            } else if second.format("%:z").to_string() == offset {
                second
            } else {
                return Err(
                    "This time occurs twice when the clocks move back. Choose the first or second occurrence.",
                );
            }
        }
    };
    Ok(date
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_full_dates_and_twelve_hour_times_in_the_profile_timezone() {
        for (utc, timezone, expected) in [
            (
                "2026-03-23T13:30:00.000Z",
                "America/New_York",
                "March 23, 2026 at 9:30 AM",
            ),
            (
                "2026-01-01T00:05:00.000Z",
                "UTC",
                "January 01, 2026 at 12:05 AM",
            ),
            (
                "2026-01-01T12:00:00.000Z",
                "UTC",
                "January 01, 2026 at 12:00 PM",
            ),
            (
                "2026-01-01T00:05:00.000Z",
                "America/New_York",
                "December 31, 2025 at 7:05 PM",
            ),
            (
                "2026-03-23T13:30:00.000Z",
                "Asia/Kathmandu",
                "March 23, 2026 at 7:15 PM",
            ),
        ] {
            assert_eq!(display(utc, timezone), expected);
        }
    }

    #[test]
    fn converts_seasonal_and_fractional_offsets_and_handles_clock_changes() {
        assert_eq!(
            to_utc("2026-07-01T12:00", "America/New_York", "").unwrap(),
            "2026-07-01T16:00:00.000Z"
        );
        assert_eq!(
            to_utc("2026-01-01T12:00", "America/New_York", "").unwrap(),
            "2026-01-01T17:00:00.000Z"
        );
        assert_eq!(
            to_utc("2026-01-01T12:00", "Asia/Kathmandu", "").unwrap(),
            "2026-01-01T06:15:00.000Z"
        );
        assert!(
            to_utc("2026-03-08T02:30", "America/New_York", "")
                .unwrap_err()
                .contains("does not exist")
        );
        assert!(
            to_utc("2026-11-01T01:30", "America/New_York", "")
                .unwrap_err()
                .contains("occurs twice")
        );
        assert_eq!(
            repeated_offsets("2026-11-01T01:30", "America/New_York"),
            ["-04:00", "-05:00"]
        );
        assert_eq!(
            to_utc("2026-11-01T01:30", "America/New_York", "-05:00").unwrap(),
            "2026-11-01T06:30:00.000Z"
        );
        assert!(to_utc("2026-02-30T01:30", "UTC", "").is_err());
    }

    #[test]
    fn relative_days_follow_local_calendar_boundaries() {
        let now = DateTime::parse_from_rfc3339("2026-07-01T02:00:00Z")
            .unwrap()
            .to_utc();
        let event = "2026-07-01T05:00:00.000Z";
        assert_eq!(days_until(event, now, "UTC"), Some(0));
        assert_eq!(days_until(event, now, "America/New_York"), Some(1));
        assert_eq!(
            days_until("2026-07-01T00:00:00Z", now, "America/New_York"),
            Some(0)
        );
        assert_eq!(
            display(event, "America/New_York"),
            "July 01, 2026 at 1:00 AM"
        );
    }
}
