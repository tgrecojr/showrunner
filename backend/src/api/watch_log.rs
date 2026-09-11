use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::db::watch_log::{self, DEFAULT_PER_PAGE, MAX_PER_PAGE};
use crate::error::{AppError, Result};
use crate::models::watch_log::WatchLogPage;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct WatchLogQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// `GET /api/v1/watch-log?page=&per_page=` — newest-first history of
/// watched/unwatched actions. `page` is 1-based; `per_page` is clamped to
/// `1..=MAX_PER_PAGE` so a caller can't request an unbounded response.
pub async fn list_watch_log(
    State(state): State<AppState>,
    Query(q): Query<WatchLogQuery>,
) -> Result<Json<WatchLogPage>> {
    let page = q.page.unwrap_or(1);
    if page < 1 {
        return Err(AppError::InvalidData("page must be >= 1".into()));
    }
    let per_page = q.per_page.unwrap_or(DEFAULT_PER_PAGE);
    if per_page < 1 {
        return Err(AppError::InvalidData("per_page must be >= 1".into()));
    }
    let per_page = per_page.min(MAX_PER_PAGE);

    let result = watch_log::list_entries(&state.pool, page, per_page).await?;
    Ok(Json(result))
}
