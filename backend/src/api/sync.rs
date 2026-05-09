use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::Result;
use crate::logic::resync;
use crate::state::AppState;

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
