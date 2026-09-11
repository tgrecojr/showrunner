//! Queries for the append-only `watch_log` table. Inserts take a transaction
//! so a log row is committed atomically with the state change it describes.

use chrono::Utc;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::error::Result;
use crate::models::show::poster_url;
use crate::models::watch_log::{NewWatchLogEntry, WatchLogEntry, WatchLogPage, WatchLogRow};

/// Upper bound on `per_page`; the handler clamps to this.
pub const MAX_PER_PAGE: i64 = 100;
pub const DEFAULT_PER_PAGE: i64 = 50;

pub async fn insert_entry(
    tx: &mut Transaction<'_, Sqlite>,
    entry: &NewWatchLogEntry<'_>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO watch_log (
            occurred_at, media_type, action, scope, tmdb_id, title, poster_path,
            season_number, episode_number, episode_name, episode_count
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&now)
    .bind(entry.media_type)
    .bind(entry.action)
    .bind(entry.scope)
    .bind(entry.tmdb_id)
    .bind(entry.title)
    .bind(entry.poster_path)
    .bind(entry.season_number)
    .bind(entry.episode_number)
    .bind(entry.episode_name)
    .bind(entry.episode_count)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn count_entries(pool: &SqlitePool) -> Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM watch_log")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Newest first. `page` is 1-based; `per_page` must already be within
/// `1..=MAX_PER_PAGE` (the handler validates).
pub async fn list_entries(pool: &SqlitePool, page: i64, per_page: i64) -> Result<WatchLogPage> {
    let per_page = per_page.clamp(1, MAX_PER_PAGE);
    let page = page.max(1);
    let offset = (page - 1) * per_page;

    let total = count_entries(pool).await?;

    let rows: Vec<WatchLogRow> = sqlx::query_as(
        "SELECT id, occurred_at, media_type, action, scope, tmdb_id, title, poster_path,
                season_number, episode_number, episode_name, episode_count
         FROM watch_log
         ORDER BY occurred_at DESC, id DESC
         LIMIT ? OFFSET ?",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let entries = rows
        .into_iter()
        .map(|r| WatchLogEntry {
            id: r.id,
            occurred_at: r.occurred_at,
            media_type: r.media_type,
            action: r.action,
            scope: r.scope,
            tmdb_id: r.tmdb_id,
            title: r.title,
            poster_url: poster_url(r.poster_path.as_deref()),
            season_number: r.season_number,
            episode_number: r.episode_number,
            episode_name: r.episode_name,
            episode_count: r.episode_count,
        })
        .collect();

    Ok(WatchLogPage {
        entries,
        page,
        per_page,
        total,
    })
}
