use chrono::Utc;
use chrono_tz::Tz;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::datasources::tmdb::{TmdbEpisode, TmdbMovie, TmdbSeason, TmdbShow};
use crate::error::Result;
use crate::models::movie::{MovieRow, MovieWatchlistItem};
use crate::models::show::{
    backdrop_url, poster_url, CalendarEpisode, EpisodeDetail, EpisodeRow, SeasonDetail, SeasonRow,
    ShowDetail, ShowRow, UpNextItem, WatchlistItem,
};
use crate::state::today_in;

/// Insert show + seasons + episodes in one transaction. Skips season 0 (Specials).
/// `season_episodes` is parallel to the seasons in `show.seasons` after filtering.
pub async fn insert_show_full(
    pool: &SqlitePool,
    show: &TmdbShow,
    seasons: &[TmdbSeason],
) -> Result<()> {
    let providers_json = serde_json::to_string(&show.us_providers())?;
    let now = Utc::now().to_rfc3339();

    let mut tx = pool.begin().await?;

    // Show
    sqlx::query(
        r#"
        INSERT INTO shows (
            tmdb_id, name, overview, poster_path, backdrop_path, status,
            first_air_date, last_air_date, in_production, watch_providers_json,
            notify_new_episodes, last_synced_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
        "#,
    )
    .bind(show.id)
    .bind(&show.name)
    .bind(empty_to_none(show.overview.as_deref()))
    .bind(empty_to_none(show.poster_path.as_deref()))
    .bind(empty_to_none(show.backdrop_path.as_deref()))
    .bind(empty_to_none(show.status.as_deref()))
    .bind(empty_to_none(show.first_air_date.as_deref()))
    .bind(empty_to_none(show.last_air_date.as_deref()))
    .bind(if show.in_production { 1_i64 } else { 0_i64 })
    .bind(&providers_json)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    // Seasons + episodes
    for season in seasons {
        sqlx::query(
            r#"
            INSERT INTO seasons (
                show_tmdb_id, season_number, name, overview, air_date, episode_count
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(show.id)
        .bind(season.season_number)
        .bind(empty_to_none(season.name.as_deref()))
        .bind(empty_to_none(season.overview.as_deref()))
        .bind(empty_to_none(season.air_date.as_deref()))
        .bind(season.episodes.len() as i64)
        .execute(&mut *tx)
        .await?;

        for ep in &season.episodes {
            insert_episode(&mut tx, show.id, season.season_number, ep).await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

async fn insert_episode(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    show_id: i64,
    season_number: i64,
    ep: &TmdbEpisode,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO episodes (
            show_tmdb_id, season_number, episode_number, tmdb_id,
            name, overview, air_date, runtime
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(show_id)
    .bind(season_number)
    .bind(ep.episode_number)
    .bind(ep.id)
    .bind(empty_to_none(ep.name.as_deref()))
    .bind(empty_to_none(ep.overview.as_deref()))
    .bind(empty_to_none(ep.air_date.as_deref()))
    .bind(ep.runtime)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn show_exists(pool: &SqlitePool, tmdb_id: i64) -> Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM shows WHERE tmdb_id = ?")
        .bind(tmdb_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

pub async fn delete_show(pool: &SqlitePool, tmdb_id: i64) -> Result<bool> {
    let result = sqlx::query("DELETE FROM shows WHERE tmdb_id = ?")
        .bind(tmdb_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_watchlist(pool: &SqlitePool, tz: Tz) -> Result<Vec<WatchlistItem>> {
    let today = today_in(tz);

    let shows: Vec<ShowRow> = sqlx::query_as(
        "SELECT tmdb_id, name, overview, poster_path, backdrop_path, status,
                first_air_date, last_air_date, in_production, watch_providers_json,
                notify_new_episodes, added_at, last_synced_at
         FROM shows ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;

    let mut items = Vec::with_capacity(shows.len());
    for s in shows {
        let counts = episode_counts(pool, s.tmdb_id, &today).await?;
        let next = next_unaired_air_date(pool, s.tmdb_id, &today).await?;
        items.push(WatchlistItem {
            tmdb_id: s.tmdb_id,
            name: s.name,
            poster_url: poster_url(s.poster_path.as_deref()),
            status: s.status,
            in_production: s.in_production != 0,
            watched_count: counts.watched,
            aired_count: counts.aired,
            total_count: counts.total,
            next_episode_air_date: next,
        });
    }
    Ok(items)
}

pub async fn get_watchlist_item(
    pool: &SqlitePool,
    tmdb_id: i64,
    tz: Tz,
) -> Result<Option<WatchlistItem>> {
    let today = today_in(tz);

    let s: Option<ShowRow> = sqlx::query_as(
        "SELECT tmdb_id, name, overview, poster_path, backdrop_path, status,
                first_air_date, last_air_date, in_production, watch_providers_json,
                notify_new_episodes, added_at, last_synced_at
         FROM shows WHERE tmdb_id = ?",
    )
    .bind(tmdb_id)
    .fetch_optional(pool)
    .await?;

    let Some(s) = s else { return Ok(None) };

    let counts = episode_counts(pool, tmdb_id, &today).await?;
    let next = next_unaired_air_date(pool, tmdb_id, &today).await?;
    Ok(Some(WatchlistItem {
        tmdb_id: s.tmdb_id,
        name: s.name,
        poster_url: poster_url(s.poster_path.as_deref()),
        status: s.status,
        in_production: s.in_production != 0,
        watched_count: counts.watched,
        aired_count: counts.aired,
        total_count: counts.total,
        next_episode_air_date: next,
    }))
}

pub async fn get_show_detail(pool: &SqlitePool, tmdb_id: i64) -> Result<Option<ShowDetail>> {
    let s: Option<ShowRow> = sqlx::query_as(
        "SELECT tmdb_id, name, overview, poster_path, backdrop_path, status,
                first_air_date, last_air_date, in_production, watch_providers_json,
                notify_new_episodes, added_at, last_synced_at
         FROM shows WHERE tmdb_id = ?",
    )
    .bind(tmdb_id)
    .fetch_optional(pool)
    .await?;

    let Some(s) = s else { return Ok(None) };

    let providers: Vec<String> = s
        .watch_providers_json
        .as_deref()
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();

    let seasons: Vec<SeasonRow> = sqlx::query_as(
        "SELECT show_tmdb_id, season_number, name, overview, air_date, episode_count
         FROM seasons WHERE show_tmdb_id = ? ORDER BY season_number",
    )
    .bind(tmdb_id)
    .fetch_all(pool)
    .await?;

    let mut season_details = Vec::with_capacity(seasons.len());
    for season in seasons {
        let episodes: Vec<EpisodeRow> = sqlx::query_as(
            "SELECT show_tmdb_id, season_number, episode_number, tmdb_id,
                    name, overview, air_date, runtime, watched, watched_at
             FROM episodes
             WHERE show_tmdb_id = ? AND season_number = ?
             ORDER BY episode_number",
        )
        .bind(tmdb_id)
        .bind(season.season_number)
        .fetch_all(pool)
        .await?;

        let watched_count = episodes.iter().filter(|e| e.watched != 0).count() as i64;
        let episode_details = episodes
            .into_iter()
            .map(|e| EpisodeDetail {
                episode_number: e.episode_number,
                name: e.name,
                overview: e.overview,
                air_date: e.air_date,
                runtime: e.runtime,
                watched: e.watched != 0,
            })
            .collect();

        season_details.push(SeasonDetail {
            season_number: season.season_number,
            name: season.name,
            overview: season.overview,
            air_date: season.air_date,
            episode_count: season.episode_count,
            watched_count,
            episodes: episode_details,
        });
    }

    Ok(Some(ShowDetail {
        tmdb_id: s.tmdb_id,
        name: s.name,
        overview: s.overview,
        poster_url: poster_url(s.poster_path.as_deref()),
        backdrop_url: backdrop_url(s.backdrop_path.as_deref()),
        status: s.status,
        in_production: s.in_production != 0,
        first_air_date: s.first_air_date,
        last_air_date: s.last_air_date,
        watch_providers: providers,
        seasons: season_details,
    }))
}

// === Movies ===

pub async fn movie_exists(pool: &SqlitePool, tmdb_id: i64) -> Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM movies WHERE tmdb_id = ?")
        .bind(tmdb_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

pub async fn insert_movie(pool: &SqlitePool, movie: &TmdbMovie) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO movies (
            tmdb_id, name, overview, poster_path, backdrop_path,
            release_date, runtime, added_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(movie.id)
    .bind(&movie.title)
    .bind(empty_to_none(movie.overview.as_deref()))
    .bind(empty_to_none(movie.poster_path.as_deref()))
    .bind(empty_to_none(movie.backdrop_path.as_deref()))
    .bind(empty_to_none(movie.release_date.as_deref()))
    .bind(movie.runtime)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_movies(pool: &SqlitePool) -> Result<Vec<MovieWatchlistItem>> {
    let rows: Vec<MovieRow> = sqlx::query_as(
        "SELECT tmdb_id, name, overview, poster_path, backdrop_path,
                release_date, runtime, added_at
         FROM movies
         ORDER BY added_at DESC, name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|m| MovieWatchlistItem {
            tmdb_id: m.tmdb_id,
            name: m.name,
            overview: m.overview,
            poster_url: poster_url(m.poster_path.as_deref()),
            backdrop_url: backdrop_url(m.backdrop_path.as_deref()),
            release_date: m.release_date,
            runtime: m.runtime,
            added_at: m.added_at,
        })
        .collect())
}

pub async fn get_movie(pool: &SqlitePool, tmdb_id: i64) -> Result<Option<MovieWatchlistItem>> {
    let row: Option<MovieRow> = sqlx::query_as(
        "SELECT tmdb_id, name, overview, poster_path, backdrop_path,
                release_date, runtime, added_at
         FROM movies WHERE tmdb_id = ?",
    )
    .bind(tmdb_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|m| MovieWatchlistItem {
        tmdb_id: m.tmdb_id,
        name: m.name,
        overview: m.overview,
        poster_url: poster_url(m.poster_path.as_deref()),
        backdrop_url: backdrop_url(m.backdrop_path.as_deref()),
        release_date: m.release_date,
        runtime: m.runtime,
        added_at: m.added_at,
    }))
}

pub async fn delete_movie(pool: &SqlitePool, tmdb_id: i64) -> Result<bool> {
    let result = sqlx::query("DELETE FROM movies WHERE tmdb_id = ?")
        .bind(tmdb_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn tracked_movie_tmdb_ids_in(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<i64>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT tmdb_id FROM movies WHERE tmdb_id IN (");
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(*id);
    }
    separated.push_unseparated(")");
    let rows: Vec<(i64,)> = qb.build_query_as().fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn tracked_tmdb_ids_in(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<i64>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    // SQLite doesn't bind arrays — build placeholders via QueryBuilder.
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT tmdb_id FROM shows WHERE tmdb_id IN (");
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(*id);
    }
    separated.push_unseparated(")");
    let rows: Vec<(i64,)> = qb.build_query_as().fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

// === Helpers ===

struct EpisodeCounts {
    watched: i64,
    aired: i64,
    total: i64,
}

async fn episode_counts(pool: &SqlitePool, tmdb_id: i64, today: &str) -> Result<EpisodeCounts> {
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN watched != 0 AND air_date IS NOT NULL AND air_date <= ? THEN 1 ELSE 0 END), 0) AS watched,
            COALESCE(SUM(CASE WHEN air_date IS NOT NULL AND air_date <= ? THEN 1 ELSE 0 END), 0) AS aired,
            COUNT(*) AS total
         FROM episodes WHERE show_tmdb_id = ?",
    )
    .bind(today)
    .bind(today)
    .bind(tmdb_id)
    .fetch_one(pool)
    .await?;
    Ok(EpisodeCounts {
        watched: row.0,
        aired: row.1,
        total: row.2,
    })
}

async fn next_unaired_air_date(
    pool: &SqlitePool,
    tmdb_id: i64,
    today: &str,
) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT air_date FROM episodes
         WHERE show_tmdb_id = ? AND air_date IS NOT NULL AND air_date > ?
         ORDER BY air_date ASC LIMIT 1",
    )
    .bind(tmdb_id)
    .bind(today)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

fn empty_to_none(s: Option<&str>) -> Option<&str> {
    s.filter(|v| !v.is_empty())
}

// === Resync upserts ===

/// Update mutable show fields from a fresh TMDB response. Does not touch
/// notify_new_episodes (that's a user setting).
pub async fn upsert_show_metadata(pool: &SqlitePool, show: &TmdbShow) -> Result<()> {
    let providers_json = serde_json::to_string(&show.us_providers())?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE shows SET
            name = ?,
            overview = ?,
            poster_path = ?,
            backdrop_path = ?,
            status = ?,
            first_air_date = ?,
            last_air_date = ?,
            in_production = ?,
            watch_providers_json = ?,
            last_synced_at = ?
         WHERE tmdb_id = ?",
    )
    .bind(&show.name)
    .bind(empty_to_none(show.overview.as_deref()))
    .bind(empty_to_none(show.poster_path.as_deref()))
    .bind(empty_to_none(show.backdrop_path.as_deref()))
    .bind(empty_to_none(show.status.as_deref()))
    .bind(empty_to_none(show.first_air_date.as_deref()))
    .bind(empty_to_none(show.last_air_date.as_deref()))
    .bind(if show.in_production { 1_i64 } else { 0_i64 })
    .bind(&providers_json)
    .bind(&now)
    .bind(show.id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn upsert_season(
    pool: &SqlitePool,
    show_tmdb_id: i64,
    season: &TmdbSeason,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO seasons (show_tmdb_id, season_number, name, overview, air_date, episode_count)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(show_tmdb_id, season_number) DO UPDATE SET
            name = excluded.name,
            overview = excluded.overview,
            air_date = excluded.air_date,
            episode_count = excluded.episode_count",
    )
    .bind(show_tmdb_id)
    .bind(season.season_number)
    .bind(empty_to_none(season.name.as_deref()))
    .bind(empty_to_none(season.overview.as_deref()))
    .bind(empty_to_none(season.air_date.as_deref()))
    .bind(season.episodes.len() as i64)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert or update an episode. On conflict, preserves `watched` and
/// `watched_at` so a TMDB resync never undoes user progress.
pub async fn upsert_episode_preserving_watched(
    pool: &SqlitePool,
    show_tmdb_id: i64,
    season_number: i64,
    ep: &TmdbEpisode,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO episodes (
            show_tmdb_id, season_number, episode_number, tmdb_id,
            name, overview, air_date, runtime
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(show_tmdb_id, season_number, episode_number) DO UPDATE SET
            tmdb_id = excluded.tmdb_id,
            name = excluded.name,
            overview = excluded.overview,
            air_date = excluded.air_date,
            runtime = excluded.runtime",
    )
    .bind(show_tmdb_id)
    .bind(season_number)
    .bind(ep.episode_number)
    .bind(ep.id)
    .bind(empty_to_none(ep.name.as_deref()))
    .bind(empty_to_none(ep.overview.as_deref()))
    .bind(empty_to_none(ep.air_date.as_deref()))
    .bind(ep.runtime)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_tracked_show_ids(pool: &SqlitePool) -> Result<Vec<i64>> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT tmdb_id FROM shows ORDER BY name COLLATE NOCASE")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

// === Notifications ===

#[derive(Debug, Clone)]
pub struct NotifiableEpisode {
    pub show_tmdb_id: i64,
    pub show_name: String,
    pub season_number: i64,
    pub episode_number: i64,
    pub episode_name: Option<String>,
    pub watch_providers: Vec<String>,
}

pub async fn list_episodes_airing_on(
    pool: &SqlitePool,
    date: &str,
) -> Result<Vec<NotifiableEpisode>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        show_tmdb_id: i64,
        show_name: String,
        watch_providers_json: Option<String>,
        season_number: i64,
        episode_number: i64,
        episode_name: Option<String>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT
            s.tmdb_id AS show_tmdb_id,
            s.name AS show_name,
            s.watch_providers_json,
            e.season_number,
            e.episode_number,
            e.name AS episode_name
         FROM episodes e
         JOIN shows s ON s.tmdb_id = e.show_tmdb_id
         WHERE e.air_date = ?
           AND s.notify_new_episodes = 1
         ORDER BY s.name COLLATE NOCASE, e.season_number, e.episode_number",
    )
    .bind(date)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let providers: Vec<String> = r
                .watch_providers_json
                .as_deref()
                .and_then(|j| serde_json::from_str(j).ok())
                .unwrap_or_default();
            NotifiableEpisode {
                show_tmdb_id: r.show_tmdb_id,
                show_name: r.show_name,
                season_number: r.season_number,
                episode_number: r.episode_number,
                episode_name: r.episode_name,
                watch_providers: providers,
            }
        })
        .collect())
}

pub async fn was_notified(
    pool: &SqlitePool,
    show_tmdb_id: i64,
    season_number: i64,
    episode_number: i64,
    channel: &str,
) -> Result<bool> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM notification_log
         WHERE show_tmdb_id = ? AND season_number = ? AND episode_number = ? AND channel = ?",
    )
    .bind(show_tmdb_id)
    .bind(season_number)
    .bind(episode_number)
    .bind(channel)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

pub async fn log_notification(
    pool: &SqlitePool,
    show_tmdb_id: i64,
    season_number: i64,
    episode_number: i64,
    channel: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO notification_log
            (show_tmdb_id, season_number, episode_number, channel)
         VALUES (?, ?, ?, ?)",
    )
    .bind(show_tmdb_id)
    .bind(season_number)
    .bind(episode_number)
    .bind(channel)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_up_next(pool: &SqlitePool, tz: Tz) -> Result<Vec<UpNextItem>> {
    let today = today_in(tz);

    #[derive(sqlx::FromRow)]
    struct Row {
        show_tmdb_id: i64,
        show_name: String,
        poster_path: Option<String>,
        season_number: i64,
        episode_number: i64,
        episode_name: Option<String>,
        episode_overview: Option<String>,
        air_date: String,
        remaining: i64,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "WITH next_eps AS (
            SELECT
                e.show_tmdb_id,
                e.season_number,
                e.episode_number,
                e.air_date,
                e.name AS episode_name,
                e.overview AS episode_overview,
                ROW_NUMBER() OVER (
                    PARTITION BY e.show_tmdb_id
                    ORDER BY e.air_date ASC, e.season_number ASC, e.episode_number ASC
                ) AS rn
            FROM episodes e
            WHERE e.watched = 0
              AND e.air_date IS NOT NULL
              AND e.air_date <= ?
        )
        SELECT
            s.tmdb_id AS show_tmdb_id,
            s.name AS show_name,
            s.poster_path,
            ne.season_number,
            ne.episode_number,
            ne.episode_name,
            ne.episode_overview,
            ne.air_date,
            (SELECT COUNT(*) FROM episodes e2
             WHERE e2.show_tmdb_id = s.tmdb_id
               AND e2.watched = 0
               AND e2.air_date IS NOT NULL
               AND e2.air_date <= ?) AS remaining
        FROM next_eps ne
        JOIN shows s ON s.tmdb_id = ne.show_tmdb_id
        WHERE ne.rn = 1
        ORDER BY ne.air_date ASC, s.name COLLATE NOCASE",
    )
    .bind(&today)
    .bind(&today)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UpNextItem {
            show_tmdb_id: r.show_tmdb_id,
            show_name: r.show_name,
            poster_url: poster_url(r.poster_path.as_deref()),
            season_number: r.season_number,
            episode_number: r.episode_number,
            episode_name: r.episode_name,
            episode_overview: r.episode_overview,
            air_date: r.air_date,
            remaining: r.remaining,
        })
        .collect())
}

pub async fn list_calendar_episodes(
    pool: &SqlitePool,
    start: &str,
    end: &str,
) -> Result<Vec<CalendarEpisode>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        show_tmdb_id: i64,
        show_name: String,
        poster_path: Option<String>,
        season_number: i64,
        episode_number: i64,
        episode_name: Option<String>,
        air_date: String,
        watched: i64,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT
            e.show_tmdb_id,
            s.name AS show_name,
            s.poster_path,
            e.season_number,
            e.episode_number,
            e.name AS episode_name,
            e.air_date AS air_date,
            e.watched
         FROM episodes e
         JOIN shows s ON s.tmdb_id = e.show_tmdb_id
         WHERE e.air_date IS NOT NULL
           AND e.air_date >= ?
           AND e.air_date <= ?
         ORDER BY e.air_date,
                  s.name COLLATE NOCASE,
                  e.season_number,
                  e.episode_number",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| CalendarEpisode {
            show_tmdb_id: r.show_tmdb_id,
            show_name: r.show_name,
            poster_url: poster_url(r.poster_path.as_deref()),
            season_number: r.season_number,
            episode_number: r.episode_number,
            episode_name: r.episode_name,
            air_date: r.air_date,
            watched: r.watched != 0,
        })
        .collect())
}

// === Episode mutations ===

/// Toggle a single episode's watched state. No air_date filtering — caller's
/// explicit choice. Returns whether the row existed.
pub async fn set_episode_watched(
    pool: &SqlitePool,
    show_tmdb_id: i64,
    season_number: i64,
    episode_number: i64,
    watched: bool,
) -> Result<bool> {
    let watched_at = if watched {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };

    let result = sqlx::query(
        "UPDATE episodes SET watched = ?, watched_at = ?
         WHERE show_tmdb_id = ? AND season_number = ? AND episode_number = ?",
    )
    .bind(if watched { 1_i64 } else { 0_i64 })
    .bind(watched_at)
    .bind(show_tmdb_id)
    .bind(season_number)
    .bind(episode_number)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

#[derive(Debug, Clone)]
pub enum BulkScope {
    All,
    Season {
        season_number: i64,
    },
    ThroughEpisode {
        season_number: i64,
        episode_number: i64,
    },
}

/// Bulk update watched state. Always restricted to aired episodes
/// (`air_date <= today`) so accidental marks don't apply to future airings.
pub async fn bulk_set_watched(
    pool: &SqlitePool,
    show_tmdb_id: i64,
    scope: &BulkScope,
    watched: bool,
    tz: Tz,
) -> Result<u64> {
    let today = today_in(tz);
    let watched_at = if watched {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };
    let watched_int: i64 = if watched { 1 } else { 0 };

    let result = match scope {
        BulkScope::All => {
            sqlx::query(
                "UPDATE episodes SET watched = ?, watched_at = ?
                 WHERE show_tmdb_id = ?
                   AND air_date IS NOT NULL AND air_date <= ?",
            )
            .bind(watched_int)
            .bind(&watched_at)
            .bind(show_tmdb_id)
            .bind(&today)
            .execute(pool)
            .await?
        }
        BulkScope::Season { season_number } => {
            sqlx::query(
                "UPDATE episodes SET watched = ?, watched_at = ?
                 WHERE show_tmdb_id = ? AND season_number = ?
                   AND air_date IS NOT NULL AND air_date <= ?",
            )
            .bind(watched_int)
            .bind(&watched_at)
            .bind(show_tmdb_id)
            .bind(*season_number)
            .bind(&today)
            .execute(pool)
            .await?
        }
        BulkScope::ThroughEpisode {
            season_number,
            episode_number,
        } => {
            sqlx::query(
                "UPDATE episodes SET watched = ?, watched_at = ?
                 WHERE show_tmdb_id = ?
                   AND air_date IS NOT NULL AND air_date <= ?
                   AND (season_number < ?
                        OR (season_number = ? AND episode_number <= ?))",
            )
            .bind(watched_int)
            .bind(&watched_at)
            .bind(show_tmdb_id)
            .bind(&today)
            .bind(*season_number)
            .bind(*season_number)
            .bind(*episode_number)
            .execute(pool)
            .await?
        }
    };

    Ok(result.rows_affected())
}
