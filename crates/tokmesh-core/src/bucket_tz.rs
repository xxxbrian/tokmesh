//! Process-wide "bucketing timezone": the timezone used to assign every usage
//! event — and "today" — to a calendar date.
//!
//! Session logs only record UTC timestamps, so "which local day did this usage
//! belong to" is always *reconstructed* at scan time. If we reconstruct it with
//! the machine's *current* timezone (`chrono::Local`), the answer changes when
//! the user travels, and events near local midnight drift between days. Combined
//! with the server's per-day "keep the max" merge, that drift double-counts
//! usage.
//!
//! To make bucketing stable regardless of where `submit` runs from, the CLI
//! pins a single IANA timezone (detected once, then persisted / synced) and
//! installs it here before any scanning. Everything that turns a timestamp into
//! a date — or computes "today" — routes through [`bucket_timezone`] so the
//! whole process agrees on one stable reference frame.
//!
//! When nothing is pinned the default is [`BucketTimezone::Local`], preserving
//! the previous machine-local behavior for embedders and tests.

use std::sync::OnceLock;

use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone};

/// The timezone every date bucket is computed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketTimezone {
    /// The machine's current system timezone (`chrono::Local`). Default when no
    /// timezone has been pinned.
    Local,
    /// A fixed IANA timezone pinned by the user. `chrono-tz` compiles the IANA
    /// rules into the binary, so this does not consult the system timezone
    /// database and stays stable across travel and the account's machines.
    Named(chrono_tz::Tz),
}

static BUCKET_TZ: OnceLock<BucketTimezone> = OnceLock::new();

/// Pin the process-wide bucketing timezone. The first call wins; later calls are
/// ignored so the reference frame is fixed for the process lifetime. Call this
/// once at startup, before any scanning.
pub fn set_bucket_timezone(tz: BucketTimezone) {
    let _ = BUCKET_TZ.set(tz);
}

/// The configured bucketing timezone, or [`BucketTimezone::Local`] when unset.
pub fn bucket_timezone() -> BucketTimezone {
    BUCKET_TZ.get().copied().unwrap_or(BucketTimezone::Local)
}

/// Parse an IANA timezone name (e.g. `"Asia/Shanghai"`) into a pinned
/// [`BucketTimezone`]. Returns `None` for names the tz database doesn't know.
pub fn parse_bucket_timezone(name: &str) -> Option<BucketTimezone> {
    name.parse::<chrono_tz::Tz>()
        .ok()
        .map(BucketTimezone::Named)
}

impl BucketTimezone {
    /// Format a Unix-millisecond timestamp as a `YYYY-MM-DD` date in this tz.
    /// Returns an empty string for timestamps the tz can't represent.
    pub fn date_of_ms(&self, timestamp_ms: i64) -> String {
        match self {
            BucketTimezone::Local => fmt_date(&chrono::Local, timestamp_ms),
            BucketTimezone::Named(tz) => fmt_date(tz, timestamp_ms),
        }
    }

    /// Format a Unix-millisecond timestamp as an `YYYY-MM-DD HH:00` hour bucket
    /// in this tz. Returns `None` for timestamps the tz can't represent.
    pub fn date_hour_of_ms(&self, timestamp_ms: i64) -> Option<String> {
        match self {
            BucketTimezone::Local => fmt_hour(&chrono::Local, timestamp_ms),
            BucketTimezone::Named(tz) => fmt_hour(tz, timestamp_ms),
        }
    }

    /// Convert a Unix-millisecond timestamp to a wall-clock datetime in this
    /// timezone. The returned value intentionally drops the timezone because
    /// report/TUI buckets store local calendar components.
    pub fn naive_datetime_of_ms(&self, timestamp_ms: i64) -> Option<NaiveDateTime> {
        match self {
            BucketTimezone::Local => naive_datetime(&chrono::Local, timestamp_ms),
            BucketTimezone::Named(tz) => naive_datetime(tz, timestamp_ms),
        }
    }

    /// Current wall-clock datetime in this timezone.
    pub fn now_naive(&self) -> NaiveDateTime {
        match self {
            BucketTimezone::Local => chrono::Local::now().naive_local(),
            BucketTimezone::Named(tz) => chrono::Utc::now().with_timezone(tz).naive_local(),
        }
    }

    /// Today's calendar date in this tz.
    pub fn today(&self) -> NaiveDate {
        match self {
            BucketTimezone::Local => chrono::Local::now().date_naive(),
            BucketTimezone::Named(tz) => chrono::Utc::now().with_timezone(tz).date_naive(),
        }
    }

    /// Today's local midnight (00:00) as Unix milliseconds in this tz. Used by
    /// today-only incremental scans to decide which files to look at.
    pub fn midnight_today_ms(&self) -> Option<i64> {
        self.start_of_day_ms(self.today())
    }

    /// First representable instant of `date` in this timezone. Normally this
    /// is midnight; for zones that skip local midnight during a DST transition,
    /// walk forward to the first valid wall-clock minute.
    pub fn start_of_day_ms(&self, date: NaiveDate) -> Option<i64> {
        match self {
            BucketTimezone::Local => {
                start_of_day_ms_with(date, |wall| chrono::Local.from_local_datetime(wall))
            }
            BucketTimezone::Named(tz) => {
                start_of_day_ms_with(date, |wall| tz.from_local_datetime(wall))
            }
        }
    }
}

fn fmt_date<Tz>(tz: &Tz, timestamp_ms: i64) -> String
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    match tz.timestamp_millis_opt(timestamp_ms) {
        LocalResult::Single(dt) => dt.format("%Y-%m-%d").to_string(),
        _ => String::new(),
    }
}

fn fmt_hour<Tz>(tz: &Tz, timestamp_ms: i64) -> Option<String>
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    match tz.timestamp_millis_opt(timestamp_ms) {
        LocalResult::Single(dt) => Some(dt.format("%Y-%m-%d %H:00").to_string()),
        _ => None,
    }
}

fn naive_datetime<Tz>(tz: &Tz, timestamp_ms: i64) -> Option<NaiveDateTime>
where
    Tz: TimeZone,
{
    match tz.timestamp_millis_opt(timestamp_ms) {
        LocalResult::Single(dt) => Some(dt.naive_local()),
        _ => None,
    }
}

fn start_of_day_ms_with<Tz, F>(date: NaiveDate, resolve: F) -> Option<i64>
where
    Tz: TimeZone,
    F: Fn(&NaiveDateTime) -> LocalResult<DateTime<Tz>>,
{
    let mut wall = date.and_hms_opt(0, 0, 0)?;
    for _ in 0..=(24 * 60) {
        match resolve(&wall) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => {
                return Some(dt.timestamp_millis());
            }
            LocalResult::None => wall += chrono::Duration::minutes(1),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn utc_ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn named_timezone_uses_pinned_iana_spring_forward_rules() {
        let tz = parse_bucket_timezone("America/New_York").unwrap();

        assert_eq!(
            tz.date_hour_of_ms(utc_ms(2024, 3, 10, 6, 59)),
            Some("2024-03-10 01:00".to_string())
        );
        assert_eq!(
            tz.date_hour_of_ms(utc_ms(2024, 3, 10, 7, 0)),
            Some("2024-03-10 03:00".to_string())
        );
    }

    #[test]
    fn named_timezone_uses_pinned_iana_fall_back_rules() {
        let tz = parse_bucket_timezone("America/New_York").unwrap();

        assert_eq!(
            tz.date_hour_of_ms(utc_ms(2024, 11, 3, 5, 30)),
            Some("2024-11-03 01:00".to_string())
        );
        assert_eq!(
            tz.date_hour_of_ms(utc_ms(2024, 11, 3, 6, 30)),
            Some("2024-11-03 01:00".to_string())
        );
    }

    #[test]
    fn named_timezone_exposes_pinned_wall_clock_components() {
        let tz = parse_bucket_timezone("Pacific/Kiritimati").unwrap();
        let wall = tz
            .naive_datetime_of_ms(utc_ms(2026, 7, 31, 12, 30))
            .unwrap();

        assert_eq!(
            wall.format("%Y-%m-%d %H:%M").to_string(),
            "2026-08-01 02:30"
        );
    }

    #[test]
    fn named_timezone_start_of_day_uses_pinned_offset() {
        let tz = parse_bucket_timezone("Pacific/Kiritimati").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();

        assert_eq!(tz.start_of_day_ms(date), Some(utc_ms(2026, 7, 31, 10, 0)));
    }

    #[test]
    fn start_of_day_walks_forward_across_a_midnight_gap() {
        let date = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap();
        let one_am = chrono::NaiveTime::from_hms_opt(1, 0, 0).unwrap();
        let result = start_of_day_ms_with::<chrono::Utc, _>(date, |wall| {
            if wall.time() < one_am {
                LocalResult::None
            } else {
                chrono::Utc.from_local_datetime(wall)
            }
        });

        assert_eq!(
            result,
            Some(
                chrono::Utc
                    .from_utc_datetime(&date.and_hms_opt(1, 0, 0).unwrap())
                    .timestamp_millis()
            )
        );
    }
}
