use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::db::queries;
use crate::error::Result;
use crate::models::show::UpNextItem;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct UpNextResponse {
    pub items: Vec<UpNextItem>,
}

// @spec AIRING-UPNEXT-001, AIRING-UPNEXT-002, AIRING-UPNEXT-003, AIRING-UPNEXT-004, AIRING-UPNEXT-005
pub async fn list_up_next(State(state): State<AppState>) -> Result<Json<UpNextResponse>> {
    let items = queries::list_up_next(&state.pool, state.tz).await?;
    Ok(Json(UpNextResponse { items }))
}
