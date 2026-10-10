//! Integration tests for `logic::resync`.

mod common;

use showrunner_backend::datasources::tmdb::TmdbClient;
use showrunner_backend::db::queries;
use showrunner_backend::logic::resync;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::*;

#[tokio::test]
async fn resync_show_updates_metadata_and_episodes_preserving_watched() {
    let pool = test_pool().await;
    insert_show(&pool, 42, "OldName", None, None, &[]).await;
    insert_season(&pool, 42, 1, 1).await;
    sqlx::query(
        "INSERT INTO episodes (show_tmdb_id, season_number, episode_number, name, watched, watched_at)
         VALUES (42, 1, 1, 'Old', 1, '2025-01-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tv/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 42, "name": "NewName", "in_production": false,
            "seasons": [
                {"season_number": 0},
                {"season_number": 1}
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/tv/42/season/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "season_number": 1, "name": "S1",
            "episodes": [
                {"id": 100, "episode_number": 1, "name": "Renamed",
                 "air_date": "2024-05-05", "runtime": 50}
            ]
        })))
        .mount(&server)
        .await;

    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());
    resync::resync_show(&pool, &tmdb, 42).await.unwrap();

    let detail =
        queries::get_show_detail(&pool, 42, &showrunner_backend::state::today_in(utc_tz()))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(detail.name, "NewName");
    assert_eq!(detail.seasons.len(), 1);
    assert_eq!(
        detail.seasons[0].episodes[0].name.as_deref(),
        Some("Renamed")
    );
    assert!(detail.seasons[0].episodes[0].watched, "watched preserved");
}

#[tokio::test]
async fn resync_show_skips_season_zero() {
    let pool = test_pool().await;
    insert_show(&pool, 7, "X", None, None, &[]).await;
    insert_season(&pool, 7, 1, 0).await;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tv/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 7, "name": "X",
            "seasons": [{"season_number": 0}, {"season_number": 1}]
        })))
        .mount(&server)
        .await;
    // Only season 1 should be requested. Season 0 mock returns 500 to verify
    // it's never called.
    Mock::given(method("GET"))
        .and(path("/tv/7/season/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "season_number": 1, "episodes": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/tv/7/season/0"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());
    resync::resync_show(&pool, &tmdb, 7).await.unwrap();
}

#[tokio::test]
async fn resync_all_reports_per_show_success_and_failure() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Good", None, None, &[]).await;
    insert_show(&pool, 2, "Bad", None, None, &[]).await;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tv/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "name": "Good", "seasons": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/tv/2"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());
    let report = resync::resync_all(&pool, &tmdb).await.unwrap();
    assert_eq!(report.shows_synced, 1);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].tmdb_id, 2);
}

#[tokio::test]
async fn resync_all_with_no_shows_is_a_noop() {
    let pool = test_pool().await;
    let server = MockServer::start().await;
    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());
    let report = resync::resync_all(&pool, &tmdb).await.unwrap();
    assert_eq!(report.shows_synced, 0);
    assert!(report.errors.is_empty());
}

// ===== VULN-005 (CWE-770) — resync fan-out ceiling =====
//
// Each show costs one TMDB call plus one per season, all carrying the
// operator's api_key, and the show count is attacker-growable via the
// unauthenticated POST /shows. Capping shows-per-run bounds the multiplier.

/// Comfortably above the ceiling so the cap is observable.
const OVER_CAP: i64 = 250;

/// The per-run ceiling. Deliberately a literal rather than a reference to
/// `resync::MAX_SHOWS_PER_RESYNC`: pinning it means raising the constant fails
/// this test instead of silently redefining what "capped" means.
const EXPECTED_CAP: usize = 100;

/// Answers `/tv/{id}` with a show whose `id` is taken from the path, so each
/// resync stamps its own row rather than all of them stamping show 1.
struct EchoShow;

impl wiremock::Respond for EchoShow {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let id: i64 = request
            .url
            .path()
            .rsplit('/')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": id, "name": format!("Show {id}"), "seasons": []
        }))
    }
}

async fn tmdb_answering_every_show() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(r"^/tv/\d+$"))
        .respond_with(EchoShow)
        .mount(&server)
        .await;
    server
}

// @spec RESYNC-RUN-001
#[tokio::test]
async fn resync_all_caps_shows_per_run() {
    let pool = test_pool().await;
    for i in 1..=OVER_CAP {
        insert_show(&pool, i, &format!("Show {i}"), None, None, &[]).await;
    }
    let server = tmdb_answering_every_show().await;
    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());

    let report = resync::resync_all(&pool, &tmdb).await.unwrap();

    let touched = report.shows_synced + report.errors.len();
    assert!(
        (touched as i64) < OVER_CAP,
        "resync touched all {touched} shows — no fan-out ceiling"
    );
    assert_eq!(
        touched, EXPECTED_CAP,
        "expected the run to stop at the shows-per-run ceiling"
    );
}

#[tokio::test]
async fn resync_all_below_the_ceiling_still_syncs_everything() {
    let pool = test_pool().await;
    for i in 1..=3 {
        insert_show(&pool, i, &format!("Show {i}"), None, None, &[]).await;
    }
    let server = tmdb_answering_every_show().await;
    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());

    let report = resync::resync_all(&pool, &tmdb).await.unwrap();

    assert_eq!(report.shows_synced, 3);
    assert!(report.errors.is_empty());
}

/// Shows whose `last_synced_at` has moved past the fixture's fixed timestamp.
async fn synced_since_fixture(pool: &sqlx::SqlitePool) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM shows WHERE last_synced_at > '2026-05-09T00:00:00Z'")
            .fetch_one(pool)
            .await
            .unwrap();
    n
}

// @spec RESYNC-RUN-005
#[tokio::test]
async fn resync_all_rotates_through_every_eligible_show_across_runs() {
    let pool = test_pool().await;
    for i in 1..=OVER_CAP {
        insert_show(&pool, i, &format!("Show {i}"), None, None, &[]).await;
    }
    let server = tmdb_answering_every_show().await;
    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());

    resync::resync_all(&pool, &tmdb).await.unwrap();
    assert_eq!(synced_since_fixture(&pool).await, EXPECTED_CAP as i64);
    resync::resync_all(&pool, &tmdb).await.unwrap();
    assert_eq!(synced_since_fixture(&pool).await, 2 * EXPECTED_CAP as i64);
    resync::resync_all(&pool, &tmdb).await.unwrap();
    assert_eq!(synced_since_fixture(&pool).await, OVER_CAP);
}

// @spec RESYNC-RUN-006
#[tokio::test]
async fn resync_all_skips_recently_synced_ended_shows() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Airing", None, Some("Returning Series"), &[]).await;
    insert_show(&pool, 2, "Ended", None, Some("Ended"), &[]).await;
    let recent = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE shows SET last_synced_at = ?")
        .bind(&recent)
        .execute(&pool)
        .await
        .unwrap();
    let server = tmdb_answering_every_show().await;
    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());

    let report = resync::resync_all(&pool, &tmdb).await.unwrap();

    assert_eq!(report.shows_synced, 1);
    let (ended_ts,): (String,) =
        sqlx::query_as("SELECT last_synced_at FROM shows WHERE tmdb_id = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ended_ts, recent, "ended show must not have been resynced");
}
