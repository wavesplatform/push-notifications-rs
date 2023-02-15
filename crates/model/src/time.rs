use std::fmt;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(i64);

pub type DateTimeUtc = DateTime<Utc>;
pub type DateTimeTz = DateTime<FixedOffset>;

impl Timestamp {
    pub fn from_unix_timestamp_millis(unix_timestamp: i64) -> Self {
        Timestamp(unix_timestamp)
    }

    pub fn unix_timestamp_millis(&self) -> i64 {
        self.0
    }

    pub fn date_time_utc(&self) -> Option<DateTimeUtc> {
        let unix_timestamp = self.unix_timestamp_millis();
        Utc.timestamp_millis_opt(unix_timestamp).earliest()
    }

    pub fn date_time(&self, utc_offset_seconds: i32) -> Option<DateTimeTz> {
        let unix_timestamp = self.unix_timestamp_millis();
        let tz = FixedOffset::east_opt(utc_offset_seconds)?;
        tz.timestamp_millis_opt(unix_timestamp).earliest()
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(dt) = self.date_time_utc() {
            write!(f, "{}", dt.format("%+")) // like "2001-07-08T00:34:60.026490+09:30"
        } else {
            write!(f, "{}", self.unix_timestamp_millis())
        }
    }
}
