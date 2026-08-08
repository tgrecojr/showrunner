use chrono::Utc;
use chrono_tz::Tz;
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::datasources::tmdb::TmdbClient;

/// Minimum wall-clock gap between two manual `POST /api/v1/sync` runs.
pub const MANUAL_SYNC_MIN_INTERVAL: Duration = Duration::from_secs(60);

/// Enforces a minimum interval between runs of an expensive operation.
///
/// `resync_all` caps how much work a single run can do; this caps how often a
/// run can be started, so the ceiling can't just be reapplied in a tight loop by
/// an unauthenticated caller. State is per-`AppState` rather than a process
/// global so tests and any future multi-instance setup stay independent.
#[derive(Debug, Default)]
pub struct RateGate {
    last_run: Mutex<Option<Instant>>,
}

impl RateGate {
    /// Record a run and return true, or return false if one happened less than
    /// `min_interval` ago. Checking and recording are one critical section, so
    /// concurrent callers can't both pass.
    pub fn try_acquire(&self, min_interval: Duration) -> bool {
        let mut last = self.last_run.lock().expect("sync gate mutex poisoned");
        let now = Instant::now();
        match *last {
            Some(prev) if now.duration_since(prev) < min_interval => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub tmdb: Arc<TmdbClient>,
    pub tz: Tz,
    pub sync_gate: Arc<RateGate>,
}

impl AppState {
    pub fn new(pool: SqlitePool, tmdb: TmdbClient, tz: Tz) -> Self {
        Self {
            pool,
            tmdb: Arc::new(tmdb),
            tz,
            sync_gate: Arc::new(RateGate::default()),
        }
    }
}

/// Today's date in the configured timezone, formatted as YYYY-MM-DD.
pub fn today_in(tz: Tz) -> String {
    Utc::now().with_timezone(&tz).date_naive().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_in_returns_iso_date() {
        let tz: Tz = "America/New_York".parse().unwrap();
        let s = today_in(tz);
        // YYYY-MM-DD shape
        assert_eq!(s.len(), 10);
        assert_eq!(s.as_bytes()[4], b'-');
        assert_eq!(s.as_bytes()[7], b'-');
        // round-trip parses
        chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").unwrap();
    }

    #[test]
    fn today_in_can_differ_across_timezones() {
        // Pacific/Kiritimati (UTC+14) and Pacific/Pago_Pago (UTC-11) span 25h,
        // so for some instants their dates differ. Just verify both parse.
        let east: Tz = "Pacific/Kiritimati".parse().unwrap();
        let west: Tz = "Pacific/Pago_Pago".parse().unwrap();
        chrono::NaiveDate::parse_from_str(&today_in(east), "%Y-%m-%d").unwrap();
        chrono::NaiveDate::parse_from_str(&today_in(west), "%Y-%m-%d").unwrap();
    }
}
