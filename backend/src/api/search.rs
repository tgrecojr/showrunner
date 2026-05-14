use std::collections::HashSet;

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::queries;
use crate::error::{AppError, Result};
use crate::state::AppState;

const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w185";

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub media_type: String,
    pub tmdb_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub date: Option<String>,
    pub poster_url: Option<String>,
    pub already_tracked: bool,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

// Raw shape from TMDB's /search/multi: each result carries a `media_type`
// discriminator and TV/movie-specific fields. Persons are filtered out.
#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbMultiResult>,
}

#[derive(Debug, Deserialize)]
struct TmdbMultiResult {
    id: i64,
    media_type: Option<String>,
    // TV uses `name` + `first_air_date`; movies use `title` + `release_date`.
    name: Option<String>,
    title: Option<String>,
    first_air_date: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
}

pub async fn search_shows(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<SearchResponse>> {
    let query = q.q.trim();
    if query.is_empty() {
        return Err(AppError::InvalidData(
            "query parameter `q` is required".into(),
        ));
    }

    let url = format!("{}/search/multi", state.tmdb.base_url());
    let resp = state
        .tmdb
        .http()
        .get(&url)
        .query(&[
            ("api_key", state.tmdb.api_key()),
            ("query", query),
            ("include_adult", "false"),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(AppError::Upstream(format!(
            "TMDB returned {}",
            resp.status()
        )));
    }

    let body: TmdbSearchResponse = resp.json().await?;

    let mut tv_ids: Vec<i64> = Vec::new();
    let mut movie_ids: Vec<i64> = Vec::new();
    for r in &body.results {
        match r.media_type.as_deref() {
            Some("tv") => tv_ids.push(r.id),
            Some("movie") => movie_ids.push(r.id),
            _ => {}
        }
    }

    let tracked_tv: HashSet<i64> = queries::tracked_tmdb_ids_in(&state.pool, &tv_ids)
        .await?
        .into_iter()
        .collect();
    let tracked_movies: HashSet<i64> = queries::tracked_movie_tmdb_ids_in(&state.pool, &movie_ids)
        .await?
        .into_iter()
        .collect();

    let results = body
        .results
        .into_iter()
        .filter_map(|r| {
            let media_type = match r.media_type.as_deref() {
                Some("tv") => "tv",
                Some("movie") => "movie",
                _ => return None,
            };
            let (name, date, already_tracked) = match media_type {
                "tv" => (
                    r.name.unwrap_or_default(),
                    r.first_air_date,
                    tracked_tv.contains(&r.id),
                ),
                _ => (
                    r.title.unwrap_or_default(),
                    r.release_date,
                    tracked_movies.contains(&r.id),
                ),
            };
            Some(SearchResult {
                media_type: media_type.to_string(),
                tmdb_id: r.id,
                name,
                overview: r.overview.filter(|v| !v.is_empty()),
                date: date.filter(|v| !v.is_empty()),
                poster_url: r
                    .poster_path
                    .filter(|v| !v.is_empty())
                    .map(|p| format!("{}{}", POSTER_BASE, p)),
                already_tracked,
            })
        })
        .collect();

    Ok(Json(SearchResponse { results }))
}
