//! Integration tests for `db::queries` against an in-memory SQLite pool.

mod common;

use chrono::{Duration, Utc};
use showrunner_backend::datasources::tmdb::{TmdbEpisode, TmdbSeason, TmdbShow};
use showrunner_backend::db::queries::{self, BulkScope};

use crate::common::*;

fn iso_today() -> String {
    Utc::now().date_naive().to_string()
}

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

    insert_show(&pool, 1, "X", None, None, true, &[]).await;
    assert!(queries::show_exists(&pool, 1).await.unwrap());
    assert!(!queries::show_exists(&pool, 99).await.unwrap());

    assert!(queries::delete_show(&pool, 1).await.unwrap());
    assert!(!queries::show_exists(&pool, 1).await.unwrap());
    assert!(!queries::delete_show(&pool, 1).await.unwrap());
}

#[tokio::test]
async fn delete_show_cascades_to_seasons_and_episodes() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "Alpha", Some("/a.jpg"), Some("Ended"), true, &[]).await;
    insert_season(&pool, 1, 1, 3).await;
    // 2 aired (one watched), 1 future
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-5)), true).await;
    insert_episode(&pool, 1, 1, 2, Some(&iso_offset(-1)), false).await;
    insert_episode(&pool, 1, 1, 3, Some(&iso_offset(7)), false).await;

    insert_show(&pool, 2, "beta", None, None, true, &[]).await;
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
        "INSERT INTO shows (tmdb_id, name, watch_providers_json, notify_new_episodes)
         VALUES (1, 'X', 'not-json', 1)",
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
    insert_show(&pool, 1, "A", None, None, true, &[]).await;
    insert_show(&pool, 5, "E", None, None, true, &[]).await;

    let empty = queries::tracked_tmdb_ids_in(&pool, &[]).await.unwrap();
    assert!(empty.is_empty());

    let mut subset = queries::tracked_tmdb_ids_in(&pool, &[1, 2, 5, 9])
        .await
        .unwrap();
    subset.sort();
    assert_eq!(subset, vec![1, 5]);
}

#[tokio::test]
async fn upsert_show_metadata_updates_existing_without_touching_notify_flag() {
    let pool = test_pool().await;
    // Start with notify off — must not be reset by upsert.
    insert_show(&pool, 7, "Old", None, Some("Returning"), false, &[]).await;

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

    let row: (String, i64, i64) = sqlx::query_as(
        "SELECT name, in_production, notify_new_episodes FROM shows WHERE tmdb_id = 7",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "New");
    assert_eq!(row.1, 0);
    assert_eq!(row.2, 0, "notify_new_episodes must be preserved");
}

#[tokio::test]
async fn upsert_season_inserts_then_updates_on_conflict() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, true, &[]).await;

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
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "Bravo", None, None, true, &[]).await;
    insert_show(&pool, 2, "alpha", None, None, true, &[]).await;
    insert_show(&pool, 3, "Charlie", None, None, true, &[]).await;
    let ids = queries::list_tracked_show_ids(&pool).await.unwrap();
    assert_eq!(ids, vec![2, 1, 3]);
}

#[tokio::test]
async fn list_episodes_airing_on_filters_by_date_and_notify_flag() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "On", None, None, true, &["Hulu", "FX"]).await;
    insert_show(&pool, 2, "Off", None, None, false, &["Netflix"]).await;
    insert_season(&pool, 1, 1, 2).await;
    insert_season(&pool, 2, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some("2026-05-09"), false).await;
    insert_episode(&pool, 1, 1, 2, Some("2026-05-10"), false).await;
    insert_episode(&pool, 2, 1, 1, Some("2026-05-09"), false).await;

    let on = queries::list_episodes_airing_on(&pool, "2026-05-09")
        .await
        .unwrap();
    assert_eq!(on.len(), 1);
    assert_eq!(on[0].show_tmdb_id, 1);
    assert_eq!(on[0].show_name, "On");
    assert_eq!(on[0].watch_providers, vec!["Hulu", "FX"]);

    let other = queries::list_episodes_airing_on(&pool, "2030-01-01")
        .await
        .unwrap();
    assert!(other.is_empty());
}

#[tokio::test]
async fn was_notified_and_log_notification_dedupes_per_channel() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_today()), false).await;

    assert!(!queries::was_notified(&pool, 1, 1, 1, "slack")
        .await
        .unwrap());
    queries::log_notification(&pool, 1, 1, 1, "slack")
        .await
        .unwrap();
    assert!(queries::was_notified(&pool, 1, 1, 1, "slack")
        .await
        .unwrap());
    // Different channel still counts as un-notified.
    assert!(!queries::was_notified(&pool, 1, 1, 1, "email")
        .await
        .unwrap());

    // Re-logging is a no-op (INSERT OR IGNORE).
    queries::log_notification(&pool, 1, 1, 1, "slack")
        .await
        .unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notification_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn list_up_next_picks_oldest_unwatched_aired_per_show() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Show1", Some("/a.jpg"), None, true, &[]).await;
    insert_show(&pool, 2, "Show2", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "Done", None, None, true, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_offset(-5)), true).await;

    let items = queries::list_up_next(&pool, utc_tz()).await.unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn list_calendar_episodes_filters_by_range() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "C", Some("/p.jpg"), None, true, &[]).await;
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
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
    let updated = queries::set_episode_watched(&pool, 1, 99, 99, true)
        .await
        .unwrap();
    assert!(!updated);
}

#[tokio::test]
async fn bulk_set_watched_all_filters_to_aired_only() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "X", None, None, true, &[]).await;
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
