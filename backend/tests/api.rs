//! End-to-end API handler tests. Drives the same Router used in production
//! against a real in-memory pool and a wiremock-backed TMDB.

mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use serde_json::Value;
use showrunner_backend::build_api_router;
use showrunner_backend::notifications::Notifier;
use sqlx::SqlitePool;
use tower::ServiceExt;
use wiremock::matchers::{method as wm_method, path as wm_path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::*;

const BODY_LIMIT: usize = 16 * 1024 * 1024;

fn iso_offset(days: i64) -> String {
    (Utc::now().date_naive() + Duration::days(days)).to_string()
}

async fn body_to_value(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), BODY_LIMIT).await.unwrap();
    if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    }
}

async fn collect_text(resp: axum::response::Response) -> String {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn empty_request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

struct TestApp {
    pool: SqlitePool,
    tmdb_server: MockServer,
    notifier_calls: Option<
        std::sync::Arc<std::sync::Mutex<Vec<showrunner_backend::notifications::NotificationEvent>>>,
    >,
    state: showrunner_backend::state::AppState,
}

async fn build_app(notifiers: Vec<Box<dyn Notifier>>) -> TestApp {
    let pool = test_pool().await;
    let tmdb_server = MockServer::start().await;
    let state = app_state(pool.clone(), tmdb_server.uri(), notifiers, utc_tz());
    TestApp {
        pool,
        tmdb_server,
        notifier_calls: None,
        state,
    }
}

async fn build_app_with_recording_notifier() -> TestApp {
    let (rec, calls) = RecordingNotifier::new("rec");
    let pool = test_pool().await;
    let tmdb_server = MockServer::start().await;
    let state = app_state(
        pool.clone(),
        tmdb_server.uri(),
        vec![Box::new(rec) as Box<dyn Notifier>],
        utc_tz(),
    );
    TestApp {
        pool,
        tmdb_server,
        notifier_calls: Some(calls),
        state,
    }
}

// ============================ Health ============================

#[tokio::test]
async fn health_returns_ok_when_db_responsive() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/health"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert_eq!(v["status"], "ok");
    assert_eq!(v["database"], true);
    assert_eq!(v["notifiers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn health_lists_configured_notifier_channels() {
    let app = build_app_with_recording_notifier().await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/health"))
        .await
        .unwrap();
    let v = body_to_value(resp).await;
    assert_eq!(v["notifiers"], serde_json::json!(["rec"]));
}

// ============================ Search ============================

#[tokio::test]
async fn search_returns_mixed_results_with_already_tracked_flag() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "TrackedShow", None, None, true, &[]).await;
    insert_movie(&app.pool, 27205, "Inception").await;

    Mock::given(wm_method("GET"))
        .and(wm_path("/search/multi"))
        .and(query_param("query", "bear"))
        .and(query_param("api_key", "test_key"))
        .and(query_param("include_adult", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "results": [
                {"id": 1, "media_type": "tv", "name": "TrackedShow", "overview": "x",
                 "first_air_date": "2020-01-01", "poster_path": "/a.jpg"},
                {"id": 2, "media_type": "tv", "name": "Other", "overview": "",
                 "first_air_date": "", "poster_path": ""},
                {"id": 27205, "media_type": "movie", "title": "Inception",
                 "overview": "dreams", "release_date": "2010-07-16",
                 "poster_path": "/p.jpg"},
                {"id": 99, "media_type": "movie", "title": "Other Movie",
                 "release_date": "2024-01-01"},
                {"id": 500, "media_type": "person", "name": "Nolan"}
            ]
        })))
        .mount(&app.tmdb_server)
        .await;

    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/search?q=bear"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    let results = v["results"].as_array().unwrap();
    // Persons are filtered out, so 4 results remain.
    assert_eq!(results.len(), 4);
    assert_eq!(results[0]["media_type"], "tv");
    assert_eq!(results[0]["already_tracked"], true);
    assert_eq!(results[0]["date"], "2020-01-01");
    assert_eq!(
        results[0]["poster_url"],
        "https://image.tmdb.org/t/p/w185/a.jpg"
    );
    assert_eq!(results[1]["media_type"], "tv");
    assert_eq!(results[1]["already_tracked"], false);
    assert_eq!(results[1]["date"], Value::Null);
    assert_eq!(results[2]["media_type"], "movie");
    assert_eq!(results[2]["name"], "Inception");
    assert_eq!(results[2]["already_tracked"], true);
    assert_eq!(results[2]["date"], "2010-07-16");
    assert_eq!(results[3]["media_type"], "movie");
    assert_eq!(results[3]["already_tracked"], false);
}

#[tokio::test]
async fn search_rejects_blank_query() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/search?q=%20%20"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_returns_502_when_tmdb_errors() {
    let app = build_app(vec![]).await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/search/multi"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&app.tmdb_server)
        .await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/search?q=bear"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
}

// ============================ Shows: list ============================

#[tokio::test]
async fn list_shows_returns_watchlist() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    insert_season(&app.pool, 1, 1, 1).await;
    insert_episode(&app.pool, 1, 1, 1, Some(&iso_offset(-1)), true).await;

    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/shows"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    let shows = v["shows"].as_array().unwrap();
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0]["name"], "X");
    assert_eq!(shows[0]["watched_count"], 1);
}

// ============================ Shows: add ============================

#[tokio::test]
async fn add_show_fetches_from_tmdb_and_inserts() {
    let app = build_app(vec![]).await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/tv/55"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 55, "name": "Added",
            "seasons": [{"season_number": 0}, {"season_number": 1}]
        })))
        .mount(&app.tmdb_server)
        .await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/tv/55/season/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "season_number": 1, "episodes": [
                {"id": 1, "episode_number": 1, "air_date": "2024-01-01"}
            ]
        })))
        .mount(&app.tmdb_server)
        .await;

    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows",
            serde_json::json!({"tmdb_id": 55}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let v = body_to_value(resp).await;
    assert_eq!(v["tmdb_id"], 55);
    assert_eq!(v["name"], "Added");
}

#[tokio::test]
async fn add_show_rejects_duplicate() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 55, "Already", None, None, true, &[]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows",
            serde_json::json!({"tmdb_id": 55}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v = body_to_value(resp).await;
    assert!(v["error"].as_str().unwrap().contains("already"));
}

#[tokio::test]
async fn add_show_returns_404_when_tmdb_missing() {
    let app = build_app(vec![]).await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/tv/9"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&app.tmdb_server)
        .await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows",
            serde_json::json!({"tmdb_id": 9}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn add_show_rejects_malformed_body() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows",
            serde_json::json!({"wrong": 1}),
        ))
        .await
        .unwrap();
    assert!(resp.status().is_client_error());
}

// ============================ Shows: get ============================

#[tokio::test]
async fn get_show_returns_detail() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    insert_season(&app.pool, 1, 1, 0).await;

    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/shows/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert_eq!(v["tmdb_id"], 1);
    assert_eq!(v["name"], "X");
}

#[tokio::test]
async fn get_show_returns_404_when_unknown() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/shows/123"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ============================ Shows: delete ============================

#[tokio::test]
async fn delete_show_removes_and_returns_204() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::DELETE, "/api/v1/shows/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_show_returns_404_for_unknown() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::DELETE, "/api/v1/shows/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ============================ Bulk watch ============================

#[tokio::test]
async fn bulk_watch_marks_all_aired_episodes() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    insert_season(&app.pool, 1, 1, 2).await;
    insert_episode(&app.pool, 1, 1, 1, Some(&iso_offset(-1)), false).await;
    insert_episode(&app.pool, 1, 1, 2, Some(&iso_offset(5)), false).await;

    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows/1/bulk-watch",
            serde_json::json!({"scope": {"type": "all"}, "watched": true}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let watched: (i64,) =
        sqlx::query_as("SELECT watched FROM episodes WHERE show_tmdb_id=1 AND episode_number=1")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let future: (i64,) =
        sqlx::query_as("SELECT watched FROM episodes WHERE show_tmdb_id=1 AND episode_number=2")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(watched.0, 1);
    assert_eq!(future.0, 0);
}

#[tokio::test]
async fn bulk_watch_supports_season_and_through_episode() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    insert_season(&app.pool, 1, 1, 2).await;
    insert_season(&app.pool, 1, 2, 2).await;
    insert_episode(&app.pool, 1, 1, 1, Some(&iso_offset(-3)), false).await;
    insert_episode(&app.pool, 1, 1, 2, Some(&iso_offset(-2)), false).await;
    insert_episode(&app.pool, 1, 2, 1, Some(&iso_offset(-1)), false).await;
    insert_episode(&app.pool, 1, 2, 2, Some(&iso_offset(5)), false).await;

    // Season scope
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows/1/bulk-watch",
            serde_json::json!({"scope": {"type": "season", "season_number": 1}, "watched": true}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM episodes WHERE watched=1 AND season_number=1")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(count.0, 2);

    // Through-episode scope
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows/1/bulk-watch",
            serde_json::json!({
                "scope": {"type": "through_episode", "season_number": 2, "episode_number": 1},
                "watched": true
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let unwatched: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodes WHERE watched=0")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(unwatched.0, 1, "only the future S02E02 stays unwatched");
}

#[tokio::test]
async fn bulk_watch_returns_404_when_show_unknown() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/shows/999/bulk-watch",
            serde_json::json!({"scope": {"type": "all"}, "watched": true}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ============================ Episodes patch ============================

#[tokio::test]
async fn patch_episode_toggles_and_returns_show_detail() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    insert_season(&app.pool, 1, 1, 1).await;
    insert_episode(&app.pool, 1, 1, 1, Some(&iso_offset(-1)), false).await;

    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::PATCH,
            "/api/v1/episodes/1/1/1",
            serde_json::json!({"watched": true}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert_eq!(v["seasons"][0]["episodes"][0]["watched"], true);
}

#[tokio::test]
async fn patch_episode_404_when_episode_missing() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::PATCH,
            "/api/v1/episodes/1/9/9",
            serde_json::json!({"watched": true}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ============================ Calendar ============================

#[tokio::test]
async fn calendar_returns_episodes_in_range() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    insert_season(&app.pool, 1, 1, 1).await;
    insert_episode(&app.pool, 1, 1, 1, Some("2026-05-15"), false).await;

    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(
            Method::GET,
            "/api/v1/calendar?start=2026-05-01&end=2026-05-31",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert_eq!(v["episodes"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn calendar_rejects_invalid_dates() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(
            Method::GET,
            "/api/v1/calendar?start=not-a-date&end=2026-05-31",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(
            Method::GET,
            "/api/v1/calendar?start=2026-05-01&end=zzz",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn calendar_rejects_inverted_range() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(
            Method::GET,
            "/api/v1/calendar?start=2026-05-30&end=2026-05-01",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let txt = collect_text(resp).await;
    assert!(txt.contains("end must be"));
}

#[tokio::test]
async fn calendar_rejects_range_exceeding_92_days() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(
            Method::GET,
            "/api/v1/calendar?start=2026-01-01&end=2026-12-31",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ============================ Up next ============================

#[tokio::test]
async fn up_next_returns_oldest_unwatched_per_show() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "X", None, None, true, &[]).await;
    insert_season(&app.pool, 1, 1, 2).await;
    insert_episode(&app.pool, 1, 1, 1, Some(&iso_offset(-5)), false).await;
    insert_episode(&app.pool, 1, 1, 2, Some(&iso_offset(-1)), false).await;

    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/up-next"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["episode_number"], 1);
    assert_eq!(items[0]["remaining"], 2);
}

// ============================ Sync ============================

#[tokio::test]
async fn sync_returns_per_show_results() {
    let app = build_app(vec![]).await;
    insert_show(&app.pool, 1, "Good", None, None, true, &[]).await;
    insert_show(&app.pool, 2, "Bad", None, None, true, &[]).await;

    Mock::given(wm_method("GET"))
        .and(wm_path("/tv/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "name": "Good", "seasons": []
        })))
        .mount(&app.tmdb_server)
        .await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/tv/2"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&app.tmdb_server)
        .await;

    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/sync",
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert_eq!(v["shows_synced"], 1);
    let errors = v["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["tmdb_id"], 2);
}

// ============================ Notifications ============================

#[tokio::test]
async fn test_notification_dispatches_to_all_channels() {
    let app = build_app_with_recording_notifier().await;

    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/notifications/test",
            serde_json::json!({"message": "ping"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let v = body_to_value(resp).await;
    let results = v["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["channel"], "rec");
    assert_eq!(results[0]["ok"], true);

    let calls = app.notifier_calls.unwrap();
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_notification_with_no_body_uses_default_message() {
    let app = build_app_with_recording_notifier().await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::POST, "/api/v1/notifications/test"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert_eq!(v["results"][0]["channel"], "rec");
}

#[tokio::test]
async fn test_notification_reports_per_channel_failures() {
    let bad = RecordingNotifier::failing("bad");
    let pool = test_pool().await;
    let server = MockServer::start().await;
    let state = app_state(
        pool.clone(),
        server.uri(),
        vec![Box::new(bad) as Box<dyn Notifier>],
        utc_tz(),
    );

    let resp = build_api_router(state)
        .oneshot(json_request(
            Method::POST,
            "/api/v1/notifications/test",
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    let results = v["results"].as_array().unwrap();
    assert_eq!(results[0]["ok"], false);
    assert!(results[0]["error"]
        .as_str()
        .unwrap()
        .contains("stub failure"));
}

#[tokio::test]
async fn test_notification_with_no_channels_returns_empty_results() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/notifications/test",
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert!(v["results"].as_array().unwrap().is_empty());
}

// ============================ Movies ============================

#[tokio::test]
async fn list_movies_returns_empty_initially() {
    let app = build_app(vec![]).await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/movies"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_to_value(resp).await;
    assert!(v["movies"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn add_movie_fetches_from_tmdb_and_inserts() {
    let app = build_app(vec![]).await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/movie/27205"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 27205,
            "title": "Inception",
            "overview": "dreams",
            "poster_path": "/p.jpg",
            "backdrop_path": "/b.jpg",
            "release_date": "2010-07-16",
            "runtime": 148
        })))
        .mount(&app.tmdb_server)
        .await;

    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/movies",
            serde_json::json!({"tmdb_id": 27205}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let v = body_to_value(resp).await;
    assert_eq!(v["tmdb_id"], 27205);
    assert_eq!(v["name"], "Inception");
    assert_eq!(v["release_date"], "2010-07-16");
    assert_eq!(v["runtime"], 148);
    assert_eq!(v["poster_url"], "https://image.tmdb.org/t/p/w185/p.jpg");

    // It also appears in the list now.
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::GET, "/api/v1/movies"))
        .await
        .unwrap();
    let v = body_to_value(resp).await;
    let movies = v["movies"].as_array().unwrap();
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0]["tmdb_id"], 27205);
}

#[tokio::test]
async fn add_movie_rejects_duplicate() {
    let app = build_app(vec![]).await;
    insert_movie(&app.pool, 1, "Already").await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/movies",
            serde_json::json!({"tmdb_id": 1}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let v = body_to_value(resp).await;
    assert!(v["error"].as_str().unwrap().contains("already"));
}

#[tokio::test]
async fn add_movie_returns_404_when_tmdb_missing() {
    let app = build_app(vec![]).await;
    Mock::given(wm_method("GET"))
        .and(wm_path("/movie/9"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&app.tmdb_server)
        .await;
    let resp = build_api_router(app.state.clone())
        .oneshot(json_request(
            Method::POST,
            "/api/v1/movies",
            serde_json::json!({"tmdb_id": 9}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_movie_removes_and_returns_204() {
    let app = build_app(vec![]).await;
    insert_movie(&app.pool, 1, "X").await;
    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::DELETE, "/api/v1/movies/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = build_api_router(app.state.clone())
        .oneshot(empty_request(Method::DELETE, "/api/v1/movies/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
