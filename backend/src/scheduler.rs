use std::time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::config::ScheduleConfig;
use crate::error::{AppError, Result};
use crate::logic::{notifications, resync};
use crate::state::AppState;

pub async fn start(state: AppState, schedule: ScheduleConfig) -> Result<JobScheduler> {
    let scheduler = JobScheduler::new()
        .await
        .map_err(|e| AppError::Config(format!("Failed to initialize scheduler: {}", e)))?;

    // Cron-driven TMDB resync — fires in the configured local timezone.
    let resync_state = state.clone();
    let resync_job =
        Job::new_async_tz(schedule.resync_cron.as_str(), state.tz, move |_uuid, _l| {
            let state = resync_state.clone();
            Box::pin(async move {
                match resync::resync_all(&state.pool, &state.tmdb).await {
                    Ok(report) => tracing::info!(
                        synced = report.shows_synced,
                        failed = report.errors.len(),
                        "scheduled resync done"
                    ),
                    Err(e) => tracing::error!(error = %e, "scheduled resync errored"),
                }
            })
        })
        .map_err(|e| AppError::Config(format!("Invalid RESYNC_CRON: {}", e)))?;

    scheduler
        .add(resync_job)
        .await
        .map_err(|e| AppError::Config(format!("Failed to add resync job: {}", e)))?;

    // Interval-driven notification check.
    let notify_state = state.clone();
    // `saturating_mul` guards against overflow on absurd configs; the value is
    // validated to be >= 1 in `Config::from_env`, so `interval` is never zero
    // (which would panic `tokio::time::interval`).
    let interval = Duration::from_secs(
        schedule
            .notification_check_interval_minutes
            .saturating_mul(60),
    );
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await; // skip immediate fire
        loop {
            ticker.tick().await;
            match notifications::check_and_notify(
                &notify_state.pool,
                &notify_state.notifier,
                notify_state.tz,
            )
            .await
            {
                Ok(0) => tracing::debug!("notification check: nothing to send"),
                Ok(sent) => tracing::info!(sent, "notification check: dispatched"),
                Err(e) => tracing::error!(error = %e, "notification check errored"),
            }
        }
    });

    scheduler
        .start()
        .await
        .map_err(|e| AppError::Config(format!("Failed to start scheduler: {}", e)))?;

    tracing::info!(
        cron = %schedule.resync_cron,
        timezone = %state.tz,
        notify_min = schedule.notification_check_interval_minutes,
        "scheduler started"
    );
    Ok(scheduler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasources::tmdb::TmdbClient;
    use crate::notifications::dispatcher::NotificationDispatcher;
    use chrono_tz::UTC;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn fresh_state() -> AppState {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!("./src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        let tmdb = TmdbClient::new("k".into());
        AppState::new(pool, tmdb, NotificationDispatcher::new(Vec::new()), UTC)
    }

    #[tokio::test]
    async fn start_succeeds_with_valid_cron_and_returns_scheduler() {
        let state = fresh_state().await;
        let schedule = ScheduleConfig {
            resync_cron: "0 0 12 * * *".into(),
            notification_check_interval_minutes: 60,
        };
        let mut scheduler = start(state, schedule).await.unwrap();
        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn start_returns_config_error_for_invalid_cron() {
        let state = fresh_state().await;
        let schedule = ScheduleConfig {
            resync_cron: "this is not a cron".into(),
            notification_check_interval_minutes: 60,
        };
        let result = start(state, schedule).await;
        match result {
            Err(AppError::Config(msg)) => {
                assert!(msg.contains("RESYNC_CRON"));
            }
            Err(other) => panic!("expected Config error, got {:?}", other),
            Ok(_) => panic!("expected Config error, got Ok"),
        }
    }
}
