use tokio_cron_scheduler::{Job, JobScheduler};

use crate::config::ScheduleConfig;
use crate::error::{AppError, Result};
use crate::logic::resync;
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

    scheduler
        .start()
        .await
        .map_err(|e| AppError::Config(format!("Failed to start scheduler: {}", e)))?;

    tracing::info!(
        cron = %schedule.resync_cron,
        timezone = %state.tz,
        "scheduler started"
    );
    Ok(scheduler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasources::tmdb::TmdbClient;
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
        AppState::new(pool, tmdb, UTC)
    }

    #[tokio::test]
    async fn start_succeeds_with_valid_cron_and_returns_scheduler() {
        let state = fresh_state().await;
        let schedule = ScheduleConfig {
            resync_cron: "0 0 12 * * *".into(),
        };
        let mut scheduler = start(state, schedule).await.unwrap();
        scheduler.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn start_returns_config_error_for_invalid_cron() {
        let state = fresh_state().await;
        let schedule = ScheduleConfig {
            resync_cron: "this is not a cron".into(),
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
