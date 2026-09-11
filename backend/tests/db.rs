//! Integration tests for `db::queries` against an in-memory SQLite pool.

mod common;

use chrono::{Duration, Utc};
use showrunner_backend::datasources::tmdb::{TmdbEpisode, TmdbSeason, TmdbShow};
use showrunner_backend::db::queries::{self, BulkScope};

use crate::common::*;

fn iso_offset(days: i64) -> String {
    (Utc::now().date_naive() + Duration::days(days)).to_string()
}

fn deser_show(json: serde_json::Value) -> TmdbShow {
    serde_json::from_value(json).unwrap()
}

fn deser_season(json: serde_json::Value) -> TmdbSeason {
    serde_json::from_value(json).unwrap()
}

#[tokio::test]
async fn insert_show_full_persists_show_seasons_and_episodes() {
    let pool = test_pool().await;

    let show = deser_show(serde_json::json!({
        "id": 100, "name": "Bear", "overview": "kitchen",
        "poster_path": "/p.jpg", "backdrop_path": "/b.jpg",
        "status": "Returning Series",
        "first_air_date": "2022-06-23", "last_air_date": "2024-06-26",
        "in_production": true,
        "watch/providers": {"results": {"US": {"flatrate": [{"provider_name": "Hulu"}]}}}
    }));

    let s1 = deser_season(serde_json::json!({
        "season_number": 1, "name": "S1", "air_date": "2022-06-23",
        "episodes": [
            {"id": 1, "episode_number": 1, "name": "P1", "air_date": "2022-06-23", "runtime": 30},
            {"id": 2, "episode_number": 2, "name": "P2", "air_date": "2022-06-24", "runtime": 32}
        ]
    }));

    queries::insert_show_full(&pool, &show, &[s1])
        .await
        .unwrap();

    let detail = queries::get_show_detail(&pool, 100).await.unwrap().unwrap();
    assert_eq!(detail.tmdb_id, 100);
    assert_eq!(detail.name, "Bear");
    assert_eq!(
        detail.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w185/p.jpg")
    );
    assert_eq!(
        detail.backdrop_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w780/b.jpg")
    );
    assert_eq!(detail.watch_providers, vec!["Hulu".to_string()]);
    assert_eq!(detail.seasons.len(), 1);
    assert_eq!(detail.seasons[0].episode_count, 2);
    assert_eq!(detail.seasons[0].episodes.len(), 2);
}

#[tokio::test]
async fn show_exists_and_delete_show() {
    let pool = test_pool().await;

    insert_show(&pool, 1, "X", None, None, &[]).await;
    assert!(queries::show_exists(&pool, 1).await.unwrap());
    assert!(!queries::show_exists(&pool, 99).await.unwrap());

    assert!(queries::delete_show(&pool, 1).await.unwrap());
    assert!(!queries::show_exists(&pool, 1).await.unwrap());
    assert!(!queries::delete_show(&pool, 1).await.unwrap());
}

#[tokio::test]
async fn delete_show_cascades_to_seasons_and_episodes() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 2).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-3)), false).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-2)), false).await;

    assert!(queries::delete_show(&pool, 1).await.unwrap());
    let detail = queries::get_show_detail(&pool, 1).await.unwrap();
    assert!(detail.is_none());
    let counts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(counts.0, 0);
}

#[tokio::test]
async fn list_watchlist_includes_progress_and_next_air_date() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Alpha", Some("/a.jpg"), Some("Ended"), &[]).await;
    insert_season(&pool, 1, 1, 3).await;
    // 2 aired (one watched), 1 future
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-5)), true).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-1)), false).await;
    insert_episode(&pool, 1, 1, 3, Some(&iso_offset(7)), false).await;

    insert_show(&pool, 2, "beta", None, None, &[]).await;
    insert_season(&pool, 2, 1, 1).await;
    insert_episode(&pool, 2, 1, 1, None, false).await;

    let items = queries::list_watchlist(&pool, utc_tz()).await.unwrap();
    // Sorted alphabetically (case-insensitive): Alpha, beta
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Alpha");
    assert_eq!(items[0].watched_count, 1);
    assert_eq!(items[0].aired_count, 2);
    assert_eq!(items[0].total_count, 3);
    assert_eq!(items[0].next_episode_air_date, Some(iso_offset(7)));

    assert_eq!(items[1].name, "beta");
    assert_eq!(items[1].watched_count, 0);
    assert_eq!(items[1].aired_count, 0);
    assert_eq!(items[1].total_count, 1);
    assert_eq!(items[1].next_episode_air_date, None);
}

#[tokio::test]
async fn get_watchlist_item_returns_none_for_unknown() {
    let pool = test_pool().await;
    assert!(queries::get_watchlist_item(&pool, 999, utc_tz())
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn get_show_detail_returns_none_for_unknown() {
    let pool = test_pool().await;
    assert!(queries::get_show_detail(&pool, 999)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn get_show_detail_handles_invalid_providers_json() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO shows (tmdb_id, name, watch_providers_json)
         VALUES (1, 'X', 'not-json')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let detail = queries::get_show_detail(&pool, 1).await.unwrap().unwrap();
    assert!(detail.watch_providers.is_empty());
}

#[tokio::test]
async fn tracked_tmdb_ids_in_handles_empty_and_subset() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "A", None, None, &[]).await;
    insert_show(&pool, 5, "E", None, None, &[]).await;

    let empty = queries::tracked_tmdb_ids_in(&pool, &[]).await.unwrap();
    assert!(empty.is_empty());

    let mut subset = queries::tracked_tmdb_ids_in(&pool, &[1, 2, 5, 9])
        .await
        .unwrap();
    subset.sort();
    assert_eq!(subset, vec![1, 5]);
}

#[tokio::test]
async fn upsert_show_metadata_updates_existing() {
    let pool = test_pool().await;
    insert_show(&pool, 7, "Old", None, Some("Returning"), &[]).await;

    let updated = deser_show(serde_json::json!({
        "id": 7, "name": "New",
        "poster_path": "/q.jpg", "status": "Ended",
        "first_air_date": "2020-01-01", "last_air_date": "2024-01-01",
        "in_production": false,
        "watch/providers": {"results": {"US": {"flatrate": [{"provider_name": "Max"}]}}}
    }));
    queries::upsert_show_metadata(&pool, &updated)
        .await
        .unwrap();

    let row: (String, i64) =
        sqlx::query_as("SELECT name, in_production FROM shows WHERE tmdb_id = 7")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, "New");
    assert_eq!(row.1, 0);
}

#[tokio::test]
async fn upsert_season_inserts_then_updates_on_conflict() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;

    let s = deser_season(serde_json::json!({
        "season_number": 2, "name": "S2", "air_date": "2024-01-01",
        "episodes": [
            {"id": 10, "episode_number": 1},
            {"id": 11, "episode_number": 2}
        ]
    }));
    queries::upsert_season(&pool, 1, &s).await.unwrap();

    let s2 = deser_season(serde_json::json!({
        "season_number": 2, "name": "S2 Renamed", "air_date": "2024-02-01",
        "episodes": [{"id": 10, "episode_number": 1}]
    }));
    queries::upsert_season(&pool, 1, &s2).await.unwrap();

    let row: (String, i64) = sqlx::query_as(
        "SELECT name, episode_count FROM seasons WHERE show_tmdb_id=1 AND season_number=2",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "S2 Renamed");
    assert_eq!(row.1, 1);
}

#[tokio::test]
async fn upsert_episode_preserves_watched_state() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    // Pre-existing watched episode
    sqlx::query(
        "INSERT INTO episodes (show_tmdb_id, season_number, episode_number,
            tmdb_id, name, watched, watched_at)
         VALUES (1, 1, 1, 99, 'Old', 1, '2025-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let ep: TmdbEpisode = serde_json::from_value(serde_json::json!({
        "id": 100, "episode_number": 1, "name": "Renamed",
        "air_date": "2024-05-05", "runtime": 45
    }))
    .unwrap();
    queries::upsert_episode_preserving_watched(&pool, 1, 1, &ep)
        .await
        .unwrap();

    let row: (
        String,
        Option<String>,
        i64,
        Option<String>,
        Option<i64>,
        Option<i64>,
    ) = sqlx::query_as(
        "SELECT name, air_date, watched, watched_at, runtime, tmdb_id
             FROM episodes WHERE show_tmdb_id=1 AND season_number=1 AND episode_number=1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "Renamed");
    assert_eq!(row.1.as_deref(), Some("2024-05-05"));
    assert_eq!(row.2, 1, "watched must remain 1 after upsert");
    assert_eq!(row.3.as_deref(), Some("2025-01-01T00:00:00Z"));
    assert_eq!(row.4, Some(45));
    assert_eq!(row.5, Some(100));
}

#[tokio::test]
async fn list_tracked_show_ids_sorted_case_insensitively() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Bravo", None, None, &[]).await;
    insert_show(&pool, 2, "alpha", None, None, &[]).await;
    insert_show(&pool, 3, "Charlie", None, None, &[]).await;
    let ids = queries::list_tracked_show_ids(&pool).await.unwrap();
    assert_eq!(ids, vec![2, 1, 3]);
}

#[tokio::test]
async fn list_up_next_picks_oldest_unwatched_aired_per_show() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Show1", Some("/a.jpg"), None, &[]).await;
    insert_show(&pool, 2, "Show2", None, None, &[]).await;
    insert_season(&pool, 1, 1, 4).await;
    insert_season(&pool, 2, 1, 2).await;

    // Show1: ep1 watched, ep2 unwatched (oldest), ep3 unwatched, ep4 future
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-30)), true).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-20)), false).await;
    insert_episode(&pool, 1, 1, 3, Some(&iso_offset(-10)), false).await;
    insert_episode(&pool, 1, 1, 4, Some(&iso_offset(10)), false).await;

    // Show2: only one unwatched aired episode
    insert_episode(&pool, 2, 1, 1, Some(&iso_offset(-5)), false).await;
    insert_episode(&pool, 2, 1, 2, Some(&iso_offset(20)), false).await;

    let items = queries::list_up_next(&pool, utc_tz()).await.unwrap();
    assert_eq!(items.len(), 2);
    // Sorted by oldest first
    assert_eq!(items[0].show_tmdb_id, 1);
    assert_eq!(items[0].episode_number, 2);
    assert_eq!(items[0].remaining, 2);
    assert_eq!(items[1].show_tmdb_id, 2);
    assert_eq!(items[1].remaining, 1);
}

#[tokio::test]
async fn list_up_next_skips_shows_with_no_unwatched_aired_episodes() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Done", None, None, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-5)), true).await;

    let items = queries::list_up_next(&pool, utc_tz()).await.unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn list_calendar_episodes_filters_by_range() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "C", Some("/p.jpg"), None, &[]).await;
    insert_season(&pool, 1, 1, 4).await;
    insert_episode(&pool, 1, 1, 1, Some("2026-05-01"), true).await;
    insert_episode(&pool, 1, 1, 2, Some("2026-05-15"), false).await;
    insert_episode(&pool, 1, 1, 3, Some("2026-06-01"), false).await;
    insert_episode(&pool, 1, 1, 4, None, false).await;

    let eps = queries::list_calendar_episodes(&pool, "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(eps.len(), 2);
    assert_eq!(eps[0].air_date, "2026-05-01");
    assert!(eps[0].watched);
    assert_eq!(eps[1].air_date, "2026-05-15");
    assert!(!eps[1].watched);
    assert_eq!(
        eps[0].poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w185/p.jpg")
    );
}

#[tokio::test]
async fn set_episode_watched_toggles_state() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-1)), false).await;

    assert!(queries::set_episode_watched(&pool, 1, 1, 1, true)
        .await
        .unwrap());
    let row: (i64, Option<String>) =
        sqlx::query_as("SELECT watched, watched_at FROM episodes WHERE show_tmdb_id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, 1);
    assert!(row.1.is_some());

    assert!(queries::set_episode_watched(&pool, 1, 1, 1, false)
        .await
        .unwrap());
    let row: (i64, Option<String>) =
        sqlx::query_as("SELECT watched, watched_at FROM episodes WHERE show_tmdb_id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, 0);
    assert_eq!(row.1, None);
}

#[tokio::test]
async fn set_episode_watched_returns_false_when_episode_missing() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    let updated = queries::set_episode_watched(&pool, 1, 99, 99, true)
        .await
        .unwrap();
    assert!(!updated);
}

#[tokio::test]
async fn bulk_set_watched_all_filters_to_aired_only() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 3).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-2)), false).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-1)), false).await;
    insert_episode(&pool, 1, 1, 3, Some(&iso_offset(10)), false).await;

    let affected = queries::bulk_set_watched(&pool, 1, &BulkScope::All, true, utc_tz())
        .await
        .unwrap();
    assert_eq!(affected, 2);

    let row: (i64,) =
        sqlx::query_as("SELECT watched FROM episodes WHERE show_tmdb_id=1 AND episode_number=3")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, 0, "future episode must remain unwatched");
}

#[tokio::test]
async fn bulk_set_watched_season_scope() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_season(&pool, 1, 2, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-2)), false).await;
    insert_episode(&pool, 1, 2, 1, Some(&iso_offset(-2)), false).await;

    let affected = queries::bulk_set_watched(
        &pool,
        1,
        &BulkScope::Season { season_number: 2 },
        true,
        utc_tz(),
    )
    .await
    .unwrap();
    assert_eq!(affected, 1);

    let s1: (i64,) =
        sqlx::query_as("SELECT watched FROM episodes WHERE show_tmdb_id=1 AND season_number=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let s2: (i64,) =
        sqlx::query_as("SELECT watched FROM episodes WHERE show_tmdb_id=1 AND season_number=2")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(s1.0, 0);
    assert_eq!(s2.0, 1);
}

#[tokio::test]
async fn bulk_set_watched_through_episode_inclusive_and_aired_only() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 3).await;
    insert_season(&pool, 1, 2, 3).await;
    // Season 1
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-10)), false).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-9)), false).await;
    insert_episode(&pool, 1, 1, 3, Some(&iso_offset(-8)), false).await;
    // Season 2: ep1 aired, ep2 aired, ep3 future
    insert_episode(&pool, 1, 2, 1, Some(&iso_offset(-3)), false).await;
    insert_episode(&pool, 1, 2, 2, Some(&iso_offset(-2)), false).await;
    insert_episode(&pool, 1, 2, 3, Some(&iso_offset(15)), false).await;

    // Mark through S02E02 — expect S01E1-3 + S02E1-2 = 5 (S02E3 future)
    let affected = queries::bulk_set_watched(
        &pool,
        1,
        &BulkScope::ThroughEpisode {
            season_number: 2,
            episode_number: 2,
        },
        true,
        utc_tz(),
    )
    .await
    .unwrap();
    assert_eq!(affected, 5);

    let unwatched: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodes WHERE watched = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unwatched.0, 1);
}

#[tokio::test]
async fn bulk_set_unwatched_clears_state() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 2).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-2)), true).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-1)), true).await;

    queries::bulk_set_watched(&pool, 1, &BulkScope::All, false, utc_tz())
        .await
        .unwrap();
    let any_watched: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodes WHERE watched=1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(any_watched.0, 0);
}

// ===== VULN-006 (CWE-400) — unbounded, unpaginated result sets =====
//
// Of the 18 SQL statements in queries.rs exactly one carried a LIMIT, so
// response size and query count on the three list endpoints grew linearly with
// an attacker-growable row count. The ceiling lives inside the query functions
// so every caller is bounded.

/// Comfortably above the ceiling so the cap is observable.
const OVER_CAP: i64 = 600;

/// The ceiling the fix must enforce. Deliberately a literal rather than a
/// reference to `queries::MAX_LIST_ROWS`: pinning the value means raising the
/// constant fails this test instead of silently redefining what "capped" means.
const EXPECTED_CAP: i64 = 500;

#[tokio::test]
async fn list_watchlist_is_capped() {
    let pool = test_pool().await;
    for i in 1..=OVER_CAP {
        insert_show(&pool, i, &format!("Show {i:04}"), None, None, &[]).await;
    }

    let items = queries::list_watchlist(&pool, utc_tz()).await.unwrap();

    assert!(
        (items.len() as i64) < OVER_CAP,
        "list_watchlist returned all {} rows — no server-enforced ceiling",
        items.len()
    );
    assert_eq!(
        items.len() as i64,
        EXPECTED_CAP,
        "expected the result set to be clamped to the server-enforced ceiling"
    );
}

#[tokio::test]
async fn list_movies_is_capped() {
    let pool = test_pool().await;
    for i in 1..=OVER_CAP {
        insert_movie(&pool, i, &format!("Movie {i:04}")).await;
    }

    let items = queries::list_movies(&pool).await.unwrap();

    assert!(
        (items.len() as i64) < OVER_CAP,
        "list_movies returned all {} rows — no server-enforced ceiling",
        items.len()
    );
    assert_eq!(items.len() as i64, EXPECTED_CAP);
}

#[tokio::test]
async fn list_up_next_is_capped() {
    let pool = test_pool().await;
    for i in 1..=OVER_CAP {
        insert_show(&pool, i, &format!("Show {i:04}"), None, None, &[]).await;
        insert_season(&pool, i, 1, 1).await;
        insert_episode(&pool, i, 1, 1, Some("2020-01-01"), false).await;
    }

    let items = queries::list_up_next(&pool, utc_tz()).await.unwrap();

    assert!(
        (items.len() as i64) < OVER_CAP,
        "list_up_next returned all {} rows — no server-enforced ceiling",
        items.len()
    );
    assert_eq!(items.len() as i64, EXPECTED_CAP);
}

/// A library below the ceiling must be returned in full and in the same order
/// as before — the cap must not change ordinary behavior.
#[tokio::test]
async fn small_library_is_returned_in_full_and_in_order() {
    let pool = test_pool().await;
    for (id, name) in [(1_i64, "Charlie"), (2, "alpha"), (3, "Bravo")] {
        insert_show(&pool, id, name, None, None, &[]).await;
    }

    let items = queries::list_watchlist(&pool, utc_tz()).await.unwrap();

    assert_eq!(items.len(), 3);
    // ORDER BY name COLLATE NOCASE
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "Bravo", "Charlie"]);
}

// ============================ Watch log ============================

#[derive(sqlx::FromRow, Debug)]
struct LogRow {
    media_type: String,
    action: String,
    scope: String,
    tmdb_id: i64,
    title: String,
    season_number: Option<i64>,
    episode_number: Option<i64>,
    episode_name: Option<String>,
    episode_count: i64,
}

async fn log_rows(pool: &sqlx::SqlitePool) -> Vec<LogRow> {
    sqlx::query_as(
        "SELECT media_type, action, scope, tmdb_id, title, season_number,
                episode_number, episode_name, episode_count
         FROM watch_log ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn set_episode_watched_logs_watch_and_unwatch() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Severance", Some("/sev.jpg"), None, &[]).await;
    insert_season(&pool, 1, 2, 1).await;
    insert_episode(&pool, 1, 2, 5, Some(&iso_offset(-1)), false).await;

    queries::set_episode_watched(&pool, 1, 2, 5, true)
        .await
        .unwrap();
    queries::set_episode_watched(&pool, 1, 2, 5, false)
        .await
        .unwrap();

    let rows = log_rows(&pool).await;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].media_type, "tv");
    assert_eq!(rows[0].action, "watched");
    assert_eq!(rows[0].scope, "episode");
    assert_eq!(rows[0].tmdb_id, 1);
    assert_eq!(rows[0].title, "Severance");
    assert_eq!(rows[0].season_number, Some(2));
    assert_eq!(rows[0].episode_number, Some(5));
    assert_eq!(rows[0].episode_name.as_deref(), Some("Ep 5"));
    assert_eq!(rows[0].episode_count, 1);
    assert_eq!(rows[1].action, "unwatched");
}

#[tokio::test]
async fn set_episode_watched_missing_episode_logs_nothing() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    assert!(!queries::set_episode_watched(&pool, 1, 9, 9, true)
        .await
        .unwrap());
    assert!(log_rows(&pool).await.is_empty());
}

#[tokio::test]
async fn bulk_set_watched_logs_one_entry_with_changed_count() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 3).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-3)), true).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-2)), false).await;
    insert_episode(&pool, 1, 1, 3, Some(&iso_offset(-1)), false).await;

    let affected = queries::bulk_set_watched(
        &pool,
        1,
        &BulkScope::Season { season_number: 1 },
        true,
        utc_tz(),
    )
    .await
    .unwrap();
    assert_eq!(affected, 2, "already-watched episode must not count");

    let rows = log_rows(&pool).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].scope, "season");
    assert_eq!(rows[0].action, "watched");
    assert_eq!(rows[0].season_number, Some(1));
    assert_eq!(rows[0].episode_number, None);
    assert_eq!(rows[0].episode_count, 2);

    // Nothing left to change: no new row, count is zero.
    let again = queries::bulk_set_watched(
        &pool,
        1,
        &BulkScope::Season { season_number: 1 },
        true,
        utc_tz(),
    )
    .await
    .unwrap();
    assert_eq!(again, 0);
    assert_eq!(log_rows(&pool).await.len(), 1);
}

#[tokio::test]
async fn bulk_set_watched_preserves_watched_at_on_already_watched() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 2).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-3)), false).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-2)), false).await;
    sqlx::query(
        "UPDATE episodes SET watched=1, watched_at='2020-01-01T00:00:00+00:00'
         WHERE episode_number=1",
    )
    .execute(&pool)
    .await
    .unwrap();

    queries::bulk_set_watched(&pool, 1, &BulkScope::All, true, utc_tz())
        .await
        .unwrap();

    let row: (String,) = sqlx::query_as("SELECT watched_at FROM episodes WHERE episode_number=1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, "2020-01-01T00:00:00+00:00");
}

#[tokio::test]
async fn bulk_scopes_map_to_log_scopes() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, &[]).await;
    insert_season(&pool, 1, 1, 2).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-3)), false).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-2)), false).await;

    queries::bulk_set_watched(
        &pool,
        1,
        &BulkScope::ThroughEpisode {
            season_number: 1,
            episode_number: 1,
        },
        true,
        utc_tz(),
    )
    .await
    .unwrap();
    queries::bulk_set_watched(&pool, 1, &BulkScope::All, false, utc_tz())
        .await
        .unwrap();

    let rows = log_rows(&pool).await;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].scope, "through_episode");
    assert_eq!(rows[0].season_number, Some(1));
    assert_eq!(rows[0].episode_number, Some(1));
    assert_eq!(rows[0].episode_count, 1);
    assert_eq!(rows[1].scope, "show");
    assert_eq!(rows[1].action, "unwatched");
    assert_eq!(rows[1].episode_count, 1);
}

#[tokio::test]
async fn mark_movie_watched_deletes_and_logs_snapshot() {
    let pool = test_pool().await;
    insert_movie(&pool, 27205, "Inception").await;

    assert!(queries::mark_movie_watched(&pool, 27205).await.unwrap());
    assert!(!queries::movie_exists(&pool, 27205).await.unwrap());

    let rows = log_rows(&pool).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].media_type, "movie");
    assert_eq!(rows[0].action, "watched");
    assert_eq!(rows[0].scope, "movie");
    assert_eq!(rows[0].tmdb_id, 27205);
    assert_eq!(rows[0].title, "Inception");

    // Second call: nothing to mark, nothing logged.
    assert!(!queries::mark_movie_watched(&pool, 27205).await.unwrap());
    assert_eq!(log_rows(&pool).await.len(), 1);
}

#[tokio::test]
async fn delete_movie_does_not_log() {
    let pool = test_pool().await;
    insert_movie(&pool, 1, "Dune").await;
    assert!(queries::delete_movie(&pool, 1).await.unwrap());
    assert!(log_rows(&pool).await.is_empty());
}

#[tokio::test]
async fn watch_log_survives_show_removal() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Gone", None, None, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-1)), false).await;
    queries::set_episode_watched(&pool, 1, 1, 1, true)
        .await
        .unwrap();
    queries::delete_show(&pool, 1).await.unwrap();

    let rows = log_rows(&pool).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Gone");
}

#[tokio::test]
async fn list_entries_paginates_newest_first() {
    use showrunner_backend::db::watch_log;

    let pool = test_pool().await;
    for i in 1..=5 {
        insert_movie(&pool, i, &format!("Movie {}", i)).await;
        queries::mark_movie_watched(&pool, i).await.unwrap();
    }
    // Force distinct, ordered timestamps regardless of clock resolution.
    for i in 1..=5 {
        sqlx::query("UPDATE watch_log SET occurred_at = ? WHERE tmdb_id = ?")
            .bind(format!("2026-09-0{}T00:00:00+00:00", i))
            .bind(i)
            .execute(&pool)
            .await
            .unwrap();
    }

    let page1 = watch_log::list_entries(&pool, 1, 2).await.unwrap();
    assert_eq!(page1.total, 5);
    assert_eq!(page1.page, 1);
    assert_eq!(page1.per_page, 2);
    let titles: Vec<_> = page1.entries.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, vec!["Movie 5", "Movie 4"]);

    let page3 = watch_log::list_entries(&pool, 3, 2).await.unwrap();
    let titles: Vec<_> = page3.entries.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, vec!["Movie 1"]);

    let page4 = watch_log::list_entries(&pool, 4, 2).await.unwrap();
    assert!(page4.entries.is_empty());
    assert_eq!(page4.total, 5);
}

#[tokio::test]
async fn list_entries_clamps_per_page() {
    use showrunner_backend::db::watch_log;

    let pool = test_pool().await;
    let page = watch_log::list_entries(&pool, 1, 10_000).await.unwrap();
    assert_eq!(page.per_page, watch_log::MAX_PER_PAGE);
}
