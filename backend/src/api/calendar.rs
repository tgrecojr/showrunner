use axum::extract::{Query, State};
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::db::queries;
use crate::error::{AppError, Result};
use crate::models::show::CalendarEpisode;
use crate::state::AppState;

const MAX_RANGE_DAYS: i64 = 92;

#[derive(Debug, Deserialize)]
pub struct CalendarQuery {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize)]
pub struct CalendarResponse {
    pub episodes: Vec<CalendarEpisode>,
}

pub async fn get_calendar(
    State(state): State<AppState>,
    Query(q): Query<CalendarQuery>,
) -> Result<Json<CalendarResponse>> {
    let start = NaiveDate::parse_from_str(&q.start, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidData(format!("invalid start date: {}", q.start)))?;
    let end = NaiveDate::parse_from_str(&q.end, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidData(format!("invalid end date: {}", q.end)))?;

    if end < start {
        return Err(AppError::InvalidData("end must be >= start".into()));
    }
    let span = (end - start).num_days();
    if span > MAX_RANGE_DAYS {
        return Err(AppError::InvalidData(format!(
            "range {} days exceeds max of {}",
            span, MAX_RANGE_DAYS
        )));
    }

    let episodes = queries::list_calendar_episodes(&state.pool, &q.start, &q.end).await?;
    Ok(Json(CalendarResponse { episodes }))
}
