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
    pub tmdb_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub first_air_date: Option<String>,
    pub poster_url: Option<String>,
    pub already_tracked: bool,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbShow>,
}

#[derive(Debug, Deserialize)]
struct TmdbShow {
    id: i64,
    name: String,
    overview: Option<String>,
    first_air_date: Option<String>,
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

    let url = format!("{}/search/tv", state.tmdb.base_url());
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

    let ids: Vec<i64> = body.results.iter().map(|s| s.id).collect();
    let tracked: HashSet<i64> = queries::tracked_tmdb_ids_in(&state.pool, &ids)
        .await?
        .into_iter()
        .collect();

    let results = body
        .results
        .into_iter()
        .map(|s| SearchResult {
            already_tracked: tracked.contains(&s.id),
            tmdb_id: s.id,
            name: s.name,
            overview: s.overview.filter(|v| !v.is_empty()),
            first_air_date: s.first_air_date.filter(|v| !v.is_empty()),
            poster_url: s
                .poster_path
                .filter(|v| !v.is_empty())
                .map(|p| format!("{}{}", POSTER_BASE, p)),
        })
        .collect();

    Ok(Json(SearchResponse { results }))
}
