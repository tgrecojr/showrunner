use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::db::queries;
use crate::error::{AppError, Result};
use crate::models::show::ShowDetail;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct EpisodeUpdate {
    pub watched: bool,
}

pub async fn patch_episode(
    State(state): State<AppState>,
    Path((show_tmdb_id, season_number, episode_number)): Path<(i64, i64, i64)>,
    Json(body): Json<EpisodeUpdate>,
) -> Result<Json<ShowDetail>> {
    let updated = queries::set_episode_watched(
        &state.pool,
        show_tmdb_id,
        season_number,
        episode_number,
        body.watched,
    )
    .await?;

    if !updated {
        return Err(AppError::NotFound(format!(
            "episode S{}E{} of show {} not found",
            season_number, episode_number, show_tmdb_id
        )));
    }

    queries::get_show_detail(&state.pool, show_tmdb_id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("show {} not on watchlist", show_tmdb_id)))
}
