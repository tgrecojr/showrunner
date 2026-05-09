use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::queries::{self, BulkScope};
use crate::error::{AppError, Result};
use crate::models::show::{AddShowRequest, ShowDetail, WatchlistItem};
use crate::state::AppState;

#[derive(Serialize)]
pub struct WatchlistResponse {
    pub shows: Vec<WatchlistItem>,
}

pub async fn list_shows(State(state): State<AppState>) -> Result<Json<WatchlistResponse>> {
    let shows = queries::list_watchlist(&state.pool, state.tz).await?;
    Ok(Json(WatchlistResponse { shows }))
}

pub async fn add_show(
    State(state): State<AppState>,
    Json(req): Json<AddShowRequest>,
) -> Result<(StatusCode, Json<WatchlistItem>)> {
    if queries::show_exists(&state.pool, req.tmdb_id).await? {
        return Err(AppError::InvalidData(format!(
            "show {} is already on the watchlist",
            req.tmdb_id
        )));
    }

    let show = state.tmdb.get_show(req.tmdb_id).await?;

    // Skip season 0 (Specials).
    let season_numbers: Vec<i64> = show
        .seasons
        .iter()
        .map(|s| s.season_number)
        .filter(|&n| n > 0)
        .collect();

    let mut seasons = Vec::with_capacity(season_numbers.len());
    for n in season_numbers {
        seasons.push(state.tmdb.get_season(req.tmdb_id, n).await?);
    }

    queries::insert_show_full(&state.pool, &show, &seasons).await?;

    let item = queries::get_watchlist_item(&state.pool, req.tmdb_id, state.tz)
        .await?
        .ok_or_else(|| AppError::Config("show vanished after insert".into()))?;
    Ok((StatusCode::CREATED, Json(item)))
}

pub async fn get_show(
    State(state): State<AppState>,
    Path(tmdb_id): Path<i64>,
) -> Result<Json<ShowDetail>> {
    queries::get_show_detail(&state.pool, tmdb_id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("show {} not on watchlist", tmdb_id)))
}

pub async fn delete_show(
    State(state): State<AppState>,
    Path(tmdb_id): Path<i64>,
) -> Result<impl IntoResponse> {
    let removed = queries::delete_show(&state.pool, tmdb_id).await?;
    if !removed {
        return Err(AppError::NotFound(format!(
            "show {} not on watchlist",
            tmdb_id
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BulkWatchScopeBody {
    All,
    Season {
        season_number: i64,
    },
    ThroughEpisode {
        season_number: i64,
        episode_number: i64,
    },
}

#[derive(Debug, Deserialize)]
pub struct BulkWatchRequest {
    pub scope: BulkWatchScopeBody,
    pub watched: bool,
}

pub async fn bulk_watch(
    State(state): State<AppState>,
    Path(tmdb_id): Path<i64>,
    Json(req): Json<BulkWatchRequest>,
) -> Result<Json<ShowDetail>> {
    if !queries::show_exists(&state.pool, tmdb_id).await? {
        return Err(AppError::NotFound(format!(
            "show {} not on watchlist",
            tmdb_id
        )));
    }

    let scope = match req.scope {
        BulkWatchScopeBody::All => BulkScope::All,
        BulkWatchScopeBody::Season { season_number } => BulkScope::Season { season_number },
        BulkWatchScopeBody::ThroughEpisode {
            season_number,
            episode_number,
        } => BulkScope::ThroughEpisode {
            season_number,
            episode_number,
        },
    };

    queries::bulk_set_watched(&state.pool, tmdb_id, &scope, req.watched, state.tz).await?;

    queries::get_show_detail(&state.pool, tmdb_id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("show {} not on watchlist", tmdb_id)))
}
