//! Shared test helpers.
//!
//! Each `tests/*.rs` file is its own crate, so this module is included via
//! `#[path = "common/mod.rs"]` or `mod common;` from each test file.

#![allow(dead_code)]

use async_trait::async_trait;
use chrono_tz::Tz;
use showrunner_backend::datasources::tmdb::TmdbClient;
use showrunner_backend::notifications::dispatcher::NotificationDispatcher;
use showrunner_backend::notifications::{NotificationEvent, Notifier};
use showrunner_backend::state::AppState;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Build a fresh in-memory SQLite pool with all migrations applied.
/// Single connection so all queries share the same in-memory database.
pub async fn test_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    sqlx::migrate!("./src/db/migrations")
        .run(&pool)
        .await
        .unwrap();
    pool
}

pub fn ny_tz() -> Tz {
    "America/New_York".parse().unwrap()
}

pub fn utc_tz() -> Tz {
    "UTC".parse().unwrap()
}

/// Notifier that records calls and lets tests inspect them.
pub struct RecordingNotifier {
    name: String,
    calls: Arc<Mutex<Vec<NotificationEvent>>>,
    fail: bool,
    counter: Arc<AtomicUsize>,
}

impl RecordingNotifier {
    pub fn new(name: &str) -> (Self, Arc<Mutex<Vec<NotificationEvent>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                name: name.to_string(),
                calls: calls.clone(),
                fail: false,
                counter: Arc::new(AtomicUsize::new(0)),
            },
            calls,
        )
    }

    pub fn failing(name: &str) -> Self {
        Self {
            name: name.to_string(),
            calls: Arc::new(Mutex::new(Vec::new())),
            fail: true,
            counter: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl Notifier for RecordingNotifier {
    fn name(&self) -> &str {
        &self.name
    }
    async fn send(&self, event: &NotificationEvent) -> showrunner_backend::error::Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.calls.lock().unwrap().push(event.clone());
        if self.fail {
            Err(showrunner_backend::error::AppError::Upstream(
                "stub failure".into(),
            ))
        } else {
            Ok(())
        }
    }
}

/// AppState wired with a real pool, a TMDB client pointed at `tmdb_base`, and
/// the given notifier list.
pub fn app_state(
    pool: SqlitePool,
    tmdb_base: String,
    notifiers: Vec<Box<dyn Notifier>>,
    tz: Tz,
) -> AppState {
    let tmdb = TmdbClient::with_base_url("test_key".into(), tmdb_base);
    AppState::new(pool, tmdb, NotificationDispatcher::new(notifiers), tz)
}

// ===== Fixture builders =====
//
// We bypass TMDB by inserting rows directly so query tests stay fast and
// deterministic. End-to-end paths through TMDB are covered separately.

pub async fn insert_show(
    pool: &SqlitePool,
    tmdb_id: i64,
    name: &str,
    poster: Option<&str>,
    status: Option<&str>,
    notify: bool,
    providers: &[&str],
) {
    let providers_json =
        serde_json::to_string(&providers.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            .unwrap();
    sqlx::query(
        "INSERT INTO shows (tmdb_id, name, poster_path, status, in_production,
            watch_providers_json, notify_new_episodes, last_synced_at)
         VALUES (?, ?, ?, ?, 1, ?, ?, '2026-05-09T00:00:00Z')",
    )
    .bind(tmdb_id)
    .bind(name)
    .bind(poster)
    .bind(status)
    .bind(&providers_json)
    .bind(if notify { 1_i64 } else { 0_i64 })
    .execute(pool)
    .await
    .unwrap();
}

pub async fn insert_season(pool: &SqlitePool, show_id: i64, season: i64, episode_count: i64) {
    sqlx::query(
        "INSERT INTO seasons (show_tmdb_id, season_number, name, episode_count)
         VALUES (?, ?, ?, ?)",
    )
    .bind(show_id)
    .bind(season)
    .bind(format!("Season {}", season))
    .bind(episode_count)
    .execute(pool)
    .await
    .unwrap();
}

pub async fn insert_movie(pool: &SqlitePool, tmdb_id: i64, name: &str) {
    sqlx::query(
        "INSERT INTO movies (tmdb_id, name, added_at)
         VALUES (?, ?, '2026-05-13T00:00:00Z')",
    )
    .bind(tmdb_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
}

pub async fn insert_episode(
    pool: &SqlitePool,
    show_id: i64,
    season: i64,
    episode: i64,
    air_date: Option<&str>,
    watched: bool,
) {
    sqlx::query(
        "INSERT INTO episodes (show_tmdb_id, season_number, episode_number,
            tmdb_id, name, air_date, watched)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(show_id)
    .bind(season)
    .bind(episode)
    .bind(show_id * 1000 + season * 100 + episode)
    .bind(format!("Ep {}", episode))
    .bind(air_date)
    .bind(if watched { 1_i64 } else { 0_i64 })
    .execute(pool)
    .await
    .unwrap();
}
