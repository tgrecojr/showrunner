use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::db::queries;
use crate::error::{AppError, Result};
use crate::models::movie::{
    profile_url, AddMovieRequest, CastMember, MovieDetail, MovieWatchlistItem,
};
use crate::state::AppState;

const CAST_LIMIT: usize = 12;

#[derive(Serialize)]
pub struct MovieListResponse {
    pub movies: Vec<MovieWatchlistItem>,
}

pub async fn list_movies(State(state): State<AppState>) -> Result<Json<MovieListResponse>> {
    let movies = queries::list_movies(&state.pool).await?;
    Ok(Json(MovieListResponse { movies }))
}

pub async fn get_movie_detail(
    State(state): State<AppState>,
    Path(tmdb_id): Path<i64>,
) -> Result<Json<MovieDetail>> {
    let item = queries::get_movie(&state.pool, tmdb_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("movie {} not on watchlist", tmdb_id)))?;

    let tmdb_movie = state.tmdb.get_movie(tmdb_id).await?;

    let mut cast_sorted: Vec<_> = tmdb_movie
        .credits
        .as_ref()
        .map(|c| c.cast.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    cast_sorted.sort_by_key(|c| c.order.unwrap_or(i64::MAX));
    let cast = cast_sorted
        .into_iter()
        .take(CAST_LIMIT)
        .map(|c| CastMember {
            name: c.name.clone(),
            character: c.character.clone().filter(|s| !s.is_empty()),
            profile_url: profile_url(c.profile_path.as_deref()),
        })
        .collect();

    Ok(Json(MovieDetail {
        tmdb_id: item.tmdb_id,
        name: item.name,
        overview: item.overview,
        poster_url: item.poster_url,
        backdrop_url: item.backdrop_url,
        release_date: item.release_date,
        runtime: item.runtime,
        watch_providers: tmdb_movie.us_providers(),
        directors: tmdb_movie.directors(),
        cast,
    }))
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
