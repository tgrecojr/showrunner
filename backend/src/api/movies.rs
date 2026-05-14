use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::db::queries;
use crate::error::{AppError, Result};
use crate::models::movie::{AddMovieRequest, MovieWatchlistItem};
use crate::state::AppState;

#[derive(Serialize)]
pub struct MovieListResponse {
    pub movies: Vec<MovieWatchlistItem>,
}

pub async fn list_movies(State(state): State<AppState>) -> Result<Json<MovieListResponse>> {
    let movies = queries::list_movies(&state.pool).await?;
    Ok(Json(MovieListResponse { movies }))
}

pub async fn add_movie(
    State(state): State<AppState>,
    Json(req): Json<AddMovieRequest>,
) -> Result<(StatusCode, Json<MovieWatchlistItem>)> {
    if queries::movie_exists(&state.pool, req.tmdb_id).await? {
        return Err(AppError::InvalidData(format!(
            "movie {} is already on the watchlist",
            req.tmdb_id
        )));
    }

    let movie = state.tmdb.get_movie(req.tmdb_id).await?;
    queries::insert_movie(&state.pool, &movie).await?;

    let item = queries::get_movie(&state.pool, req.tmdb_id)
        .await?
        .ok_or_else(|| AppError::Config("movie vanished after insert".into()))?;
    Ok((StatusCode::CREATED, Json(item)))
}

// Handles both "mark watched" and "remove" — both delete the row.
pub async fn delete_movie(
    State(state): State<AppState>,
    Path(tmdb_id): Path<i64>,
) -> Result<impl IntoResponse> {
    let removed = queries::delete_movie(&state.pool, tmdb_id).await?;
    if !removed {
        return Err(AppError::NotFound(format!(
            "movie {} not on watchlist",
            tmdb_id
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}
