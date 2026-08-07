use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::logic::resync;
use crate::state::{AppState, MANUAL_SYNC_MIN_INTERVAL};

#[derive(Debug, Serialize)]
pub struct SyncError {
    pub tmdb_id: i64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub shows_synced: usize,
    pub errors: Vec<SyncError>,
}

pub async fn manual_sync(State(state): State<AppState>) -> Result<Json<SyncResponse>> {
    // resync_all caps how much work one run does; this caps how often a run can
    // start, so an unauthenticated caller can't reapply that ceiling in a loop.
    if !state.sync_gate.try_acquire(MANUAL_SYNC_MIN_INTERVAL) {
        return Err(AppError::TooManyRequests(format!(
            "a sync ran less than {}s ago; try again shortly",
            MANUAL_SYNC_MIN_INTERVAL.as_secs()
        )));
    }

    let report = resync::resync_all(&state.pool, &state.tmdb).await?;
    Ok(Json(SyncResponse {
        shows_synced: report.shows_synced,
        errors: report
            .errors
            .into_iter()
            .map(|e| SyncError {
                tmdb_id: e.tmdb_id,
                message: e.message,
            })
            .collect(),
    }))
}
