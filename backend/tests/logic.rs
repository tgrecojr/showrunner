//! Integration tests for `logic::resync` and `logic::notifications`.

mod common;

use chrono::Utc;
use showrunner_backend::datasources::tmdb::TmdbClient;
use showrunner_backend::db::queries;
use showrunner_backend::logic::{notifications, resync};
use showrunner_backend::notifications::dispatcher::NotificationDispatcher;
use showrunner_backend::notifications::Notifier;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::*;

fn iso_today() -> String {
    Utc::now().date_naive().to_string()
}

#[tokio::test]
async fn resync_show_updates_metadata_and_episodes_preserving_watched() {
    let pool = test_pool().await;
    insert_show(&pool, 42, "OldName", None, None, true, &[]).await;
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

    let detail = queries::get_show_detail(&pool, 42).await.unwrap().unwrap();
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
    insert_show(&pool, 7, "X", None, None, true, &[]).await;
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
    insert_show(&pool, 1, "Good", None, None, true, &[]).await;
    insert_show(&pool, 2, "Bad", None, None, true, &[]).await;

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

#[tokio::test]
async fn check_and_notify_with_no_dispatchers_returns_zero() {
    let pool = test_pool().await;
    let dispatcher = NotificationDispatcher::new(Vec::new());
    let sent = notifications::check_and_notify(&pool, &dispatcher, utc_tz())
        .await
        .unwrap();
    assert_eq!(sent, 0);
}

#[tokio::test]
async fn check_and_notify_dispatches_today_episodes_and_logs() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Show", None, None, true, &["Hulu"]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_today()), false).await;

    let (rec, calls) = RecordingNotifier::new("rec");
    let dispatcher: NotificationDispatcher =
        NotificationDispatcher::new(vec![Box::new(rec) as Box<dyn Notifier>]);

    let sent = notifications::check_and_notify(&pool, &dispatcher, utc_tz())
        .await
        .unwrap();
    assert_eq!(sent, 1);
    assert_eq!(calls.lock().unwrap().len(), 1);

    // Second call: already logged, nothing new sent.
    let sent2 = notifications::check_and_notify(&pool, &dispatcher, utc_tz())
        .await
        .unwrap();
    assert_eq!(sent2, 0);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn check_and_notify_skips_shows_with_notify_flag_off() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Quiet", None, None, false, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_today()), false).await;

    let (rec, calls) = RecordingNotifier::new("rec");
    let dispatcher = NotificationDispatcher::new(vec![Box::new(rec) as Box<dyn Notifier>]);

    let sent = notifications::check_and_notify(&pool, &dispatcher, utc_tz())
        .await
        .unwrap();
    assert_eq!(sent, 0);
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn check_and_notify_does_not_log_when_send_fails_so_it_retries() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Show", None, None, true, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_today()), false).await;

    let bad = RecordingNotifier::failing("bad");
    let dispatcher = NotificationDispatcher::new(vec![Box::new(bad) as Box<dyn Notifier>]);

    let sent = notifications::check_and_notify(&pool, &dispatcher, utc_tz())
        .await
        .unwrap();
    assert_eq!(sent, 0);
    // Not logged because send failed
    assert!(!queries::was_notified(&pool, 1, 1, 1, "bad").await.unwrap());
}

#[tokio::test]
async fn check_and_notify_dispatches_independently_per_channel() {
    let pool = test_pool().await;
    insert_show(&pool, 1, "Show", None, None, true, &[]).await;
    insert_season(&pool, 1, 1, 1).await;
    insert_episode(&pool, 1, 1, 1, Some(&iso_today()), false).await;

    let (a, a_calls) = RecordingNotifier::new("a");
    let bad = RecordingNotifier::failing("b");
    let dispatcher: NotificationDispatcher = NotificationDispatcher::new(vec![
        Box::new(a) as Box<dyn Notifier>,
        Box::new(bad) as Box<dyn Notifier>,
    ]);

    let sent = notifications::check_and_notify(&pool, &dispatcher, utc_tz())
        .await
        .unwrap();
    assert_eq!(sent, 1);
    assert_eq!(a_calls.lock().unwrap().len(), 1);
    assert!(queries::was_notified(&pool, 1, 1, 1, "a").await.unwrap());
    assert!(!queries::was_notified(&pool, 1, 1, 1, "b").await.unwrap());
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

async fn tmdb_answering_every_show() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(r"^/tv/\d+$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": 1, "name": "S", "seasons": []})),
        )
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn resync_all_caps_shows_per_run() {
    let pool = test_pool().await;
    for i in 1..=OVER_CAP {
        insert_show(&pool, i, &format!("Show {i}"), None, None, true, &[]).await;
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
        insert_show(&pool, i, &format!("Show {i}"), None, None, true, &[]).await;
    }
    let server = tmdb_answering_every_show().await;
    let tmdb = TmdbClient::with_base_url("k".into(), server.uri());

    let report = resync::resync_all(&pool, &tmdb).await.unwrap();

    assert_eq!(report.shows_synced, 3);
    assert!(report.errors.is_empty());
}
