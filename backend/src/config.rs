use std::str::FromStr;

use chrono_tz::Tz;

use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database_url: String,
    pub tmdb_api_key: String,
    pub slack_webhook_url: Option<String>,
    pub schedule: ScheduleConfig,
    pub timezone: Tz,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_allowed_origin: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub resync_cron: String,
    pub notification_check_interval_minutes: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port: u16 = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse()
            .map_err(|_| AppError::Config("SERVER_PORT must be a u16".into()))?;
        let cors_allowed_origin = optional("CORS_ALLOWED_ORIGIN");

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///data/showrunner.db".to_string());

        let tmdb_api_key = std::env::var("TMDB_API_KEY")
            .map_err(|_| AppError::Config("TMDB_API_KEY is required".into()))?;
        if tmdb_api_key.trim().is_empty() {
            return Err(AppError::Config("TMDB_API_KEY is required".into()));
        }

        let slack_webhook_url = optional("SLACK_WEBHOOK_URL");

        let resync_cron =
            std::env::var("RESYNC_CRON").unwrap_or_else(|_| "0 0 6 * * *".to_string());
        let notification_check_interval_minutes: u64 =
            std::env::var("NOTIFICATION_CHECK_INTERVAL_MINUTES")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .map_err(|_| {
                    AppError::Config("NOTIFICATION_CHECK_INTERVAL_MINUTES must be a u64".into())
                })?;

        let tz_name = std::env::var("TIMEZONE").unwrap_or_else(|_| "America/New_York".to_string());
        let timezone = Tz::from_str(&tz_name).map_err(|_| {
            AppError::Config(format!(
                "TIMEZONE '{}' is not a valid IANA zone (e.g. America/New_York, Europe/London)",
                tz_name
            ))
        })?;

        Ok(Self {
            server: ServerConfig {
                host,
                port,
                cors_allowed_origin,
            },
            database_url,
            tmdb_api_key,
            slack_webhook_url,
            schedule: ScheduleConfig {
                resync_cron,
                notification_check_interval_minutes,
            },
            timezone,
        })
    }
}

fn optional(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    const ALL_KEYS: &[&str] = &[
        "SERVER_HOST",
        "SERVER_PORT",
        "CORS_ALLOWED_ORIGIN",
        "DATABASE_URL",
        "TMDB_API_KEY",
        "SLACK_WEBHOOK_URL",
        "RESYNC_CRON",
        "NOTIFICATION_CHECK_INTERVAL_MINUTES",
        "TIMEZONE",
    ];

    /// SAFETY: tests that touch process env are gated by `#[serial]`.
    fn clear_env() {
        for k in ALL_KEYS {
            unsafe { std::env::remove_var(k) };
        }
    }

    fn set(key: &str, value: &str) {
        unsafe { std::env::set_var(key, value) };
    }

    #[test]
    #[serial]
    fn defaults_apply_when_only_required_vars_set() {
        clear_env();
        set("TMDB_API_KEY", "abc");
        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.server.host, "0.0.0.0");
        assert_eq!(cfg.server.port, 3001);
        assert_eq!(cfg.server.cors_allowed_origin, None);
        assert_eq!(cfg.database_url, "sqlite:///data/showrunner.db");
        assert_eq!(cfg.tmdb_api_key, "abc");
        assert_eq!(cfg.slack_webhook_url, None);
        assert_eq!(cfg.schedule.resync_cron, "0 0 6 * * *");
        assert_eq!(cfg.schedule.notification_check_interval_minutes, 60);
        assert_eq!(cfg.timezone.name(), "America/New_York");
    }

    #[test]
    #[serial]
    fn overrides_pick_up_env_vars() {
        clear_env();
        set("TMDB_API_KEY", "k");
        set("SERVER_HOST", "127.0.0.1");
        set("SERVER_PORT", "8080");
        set("CORS_ALLOWED_ORIGIN", "https://example.com");
        set("DATABASE_URL", "sqlite::memory:");
        set("SLACK_WEBHOOK_URL", "https://hooks/x");
        set("RESYNC_CRON", "0 30 5 * * *");
        set("NOTIFICATION_CHECK_INTERVAL_MINUTES", "15");
        set("TIMEZONE", "Europe/London");

        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(
            cfg.server.cors_allowed_origin.as_deref(),
            Some("https://example.com")
        );
        assert_eq!(cfg.database_url, "sqlite::memory:");
        assert_eq!(cfg.slack_webhook_url.as_deref(), Some("https://hooks/x"));
        assert_eq!(cfg.schedule.resync_cron, "0 30 5 * * *");
        assert_eq!(cfg.schedule.notification_check_interval_minutes, 15);
        assert_eq!(cfg.timezone.name(), "Europe/London");
    }

    #[test]
    #[serial]
    fn missing_tmdb_api_key_fails() {
        clear_env();
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
        assert!(err.to_string().contains("TMDB_API_KEY"));
    }

    #[test]
    #[serial]
    fn empty_tmdb_api_key_fails() {
        clear_env();
        set("TMDB_API_KEY", "   ");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
    }

    #[test]
    #[serial]
    fn invalid_port_fails() {
        clear_env();
        set("TMDB_API_KEY", "k");
        set("SERVER_PORT", "999999");
        let err = Config::from_env().unwrap_err();
        assert!(err.to_string().contains("SERVER_PORT"));
    }

    #[test]
    #[serial]
    fn invalid_notification_interval_fails() {
        clear_env();
        set("TMDB_API_KEY", "k");
        set("NOTIFICATION_CHECK_INTERVAL_MINUTES", "not-a-num");
        let err = Config::from_env().unwrap_err();
        assert!(err
            .to_string()
            .contains("NOTIFICATION_CHECK_INTERVAL_MINUTES"));
    }

    #[test]
    #[serial]
    fn invalid_timezone_fails() {
        clear_env();
        set("TMDB_API_KEY", "k");
        set("TIMEZONE", "Mars/Olympus_Mons");
        let err = Config::from_env().unwrap_err();
        assert!(err.to_string().contains("TIMEZONE"));
    }

    #[test]
    #[serial]
    fn empty_optional_vars_treated_as_absent() {
        clear_env();
        set("TMDB_API_KEY", "k");
        set("SLACK_WEBHOOK_URL", "");
        set("CORS_ALLOWED_ORIGIN", "   ");
        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.slack_webhook_url, None);
        assert_eq!(cfg.server.cors_allowed_origin, None);
    }
}
