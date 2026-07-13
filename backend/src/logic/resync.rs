use std::time::Instant;

use crate::datasources::tmdb::TmdbClient;
use crate::db::queries;
use crate::error::Result;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct ResyncReport {
    pub shows_synced: usize,
    pub errors: Vec<ResyncError>,
}

#[derive(Debug, Clone)]
pub struct ResyncError {
    pub tmdb_id: i64,
    pub message: String,
}

pub async fn resync_show(pool: &SqlitePool, tmdb: &TmdbClient, tmdb_id: i64) -> Result<()> {
    let show = tmdb.get_show(tmdb_id).await?;
    queries::upsert_show_metadata(pool, &show).await?;

    for season_summary in &show.seasons {
        if season_summary.season_number == 0 {
            continue; // Skip Specials, same as initial add.
        }
        let season = tmdb
            .get_season(tmdb_id, season_summary.season_number)
            .await?;
        queries::upsert_season(pool, tmdb_id, &season).await?;
        for ep in &season.episodes {
            queries::upsert_episode_preserving_watched(pool, tmdb_id, season.season_number, ep)
                .await?;
        }
    }

    Ok(())
}

pub async fn resync_all(pool: &SqlitePool, tmdb: &TmdbClient) -> Result<ResyncReport> {
    let started = Instant::now();
    let ids = queries::list_tracked_show_ids(pool).await?;
    tracing::info!(count = ids.len(), "resync starting");

    let mut errors = Vec::new();
    let mut synced = 0_usize;
    for id in &ids {
        match resync_show(pool, tmdb, *id).await {
            Ok(()) => {
                synced += 1;
                tracing::info!(tmdb_id = id, "resynced");
            }
            Err(e) => {
                tracing::warn!(tmdb_id = id, error = %e, "resync failed");
                errors.push(ResyncError {
                    tmdb_id: *id,
                    // Client-safe: this string is serialized into the /sync
                    // response body, so it must not carry internal detail.
                    message: e.client_message(),
                });
            }
        }
    }

    tracing::info!(
        synced,
        failed = errors.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "resync complete"
    );
    Ok(ResyncReport {
        shows_synced: synced,
        errors,
    })
}
