use chrono::Utc;
use chrono_tz::Tz;
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::datasources::tmdb::TmdbClient;
use crate::notifications::dispatcher::NotificationDispatcher;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub tmdb: Arc<TmdbClient>,
    pub notifier: Arc<NotificationDispatcher>,
    pub tz: Tz,
}

impl AppState {
    pub fn new(
        pool: SqlitePool,
        tmdb: TmdbClient,
        notifier: NotificationDispatcher,
        tz: Tz,
    ) -> Self {
        Self {
            pool,
            tmdb: Arc::new(tmdb),
            notifier: Arc::new(notifier),
            tz,
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
