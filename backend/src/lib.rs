pub mod api;
pub mod config;
pub mod datasources;
pub mod db;
pub mod error;
pub mod logic;
pub mod models;
pub mod notifications;
pub mod scheduler;
pub mod state;

use crate::config::Config;
use crate::datasources::tmdb::TmdbClient;
use crate::db::pool::create_pool;
use crate::notifications::dispatcher::NotificationDispatcher;
use crate::notifications::slack::SlackNotifier;
use crate::notifications::Notifier;
use crate::state::AppState;
use axum::routing::{get, patch, post};
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::EnvFilter;

/// Build the API router with all routes wired up. Public so tests can drive
/// the same Router used in production.
pub fn build_api_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(api::health::health_check))
        .route("/api/v1/search", get(api::search::search_shows))
        .route(
            "/api/v1/shows",
            get(api::shows::list_shows).post(api::shows::add_show),
        )
        .route(
            "/api/v1/shows/{tmdb_id}",
            get(api::shows::get_show).delete(api::shows::delete_show),
        )
        .route(
            "/api/v1/shows/{tmdb_id}/bulk-watch",
            post(api::shows::bulk_watch),
        )
        .route(
            "/api/v1/episodes/{show_tmdb_id}/{season_number}/{episode_number}",
            patch(api::episodes::patch_episode),
        )
        .route("/api/v1/calendar", get(api::calendar::get_calendar))
        .route("/api/v1/up-next", get(api::up_next::list_up_next))
        .route("/api/v1/sync", post(api::sync::manual_sync))
        .route(
            "/api/v1/notifications/test",
            post(api::notifications::test_notification),
        )
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .with_state(state)
}

pub fn build_cors_layer(config: &Config) -> CorsLayer {
    use axum::http::Method;

    let methods = vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
    ];

    match config.server.cors_allowed_origin.as_deref() {
        Some("*") => {
            tracing::warn!("CORS configured to allow all origins");
            CorsLayer::permissive()
        }
        Some(origin) => {
            tracing::info!(origin = %origin, "CORS restricted to configured origin");
            CorsLayer::new()
                .allow_origin(
                    origin
                        .parse::<axum::http::HeaderValue>()
                        .expect("Invalid CORS_ALLOWED_ORIGIN"),
                )
                .allow_methods(methods)
                .allow_headers(tower_http::cors::Any)
        }
        None => {
            let default_origin = format!("http://localhost:{}", config.server.port);
            tracing::info!(origin = %default_origin, "CORS defaulting to localhost origin");
            CorsLayer::new()
                .allow_origin(default_origin.parse::<axum::http::HeaderValue>().unwrap())
                .allow_methods(methods)
                .allow_headers(tower_http::cors::Any)
        }
    }
}

/// Run the full server. Called from `main.rs`.
pub async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;
    tracing::info!("Configuration loaded");

    let pool = create_pool(&config.database_url).await?;

    let tmdb = TmdbClient::new(config.tmdb_api_key.clone());

    let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();
    if let Some(url) = config.slack_webhook_url.clone() {
        tracing::info!("Slack notifier enabled");
        notifiers.push(Box::new(SlackNotifier::new(url)));
    } else {
        tracing::info!("No notifiers configured — notifications disabled");
    }
    let notifier = NotificationDispatcher::new(notifiers);

    let state = AppState::new(pool, tmdb, notifier, config.timezone);

    let _scheduler = scheduler::start(state.clone(), config.schedule.clone()).await?;

    let app = build_api_router(state).layer(build_cors_layer(&config));

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "./static".to_string());
    let static_path = PathBuf::from(&static_dir);
    let app = if static_path.join("index.html").exists() {
        let index_file = static_path.join("index.html");
        let serve_dir = ServeDir::new(&static_path).not_found_service(ServeFile::new(&index_file));
        tracing::info!("Serving static files from {}", static_dir);
        app.fallback_service(serve_dir)
    } else {
        tracing::warn!("Static directory '{}' not found, API-only mode", static_dir);
        app
    };

    let addr = SocketAddr::new(
        config.server.host.parse().unwrap_or([0, 0, 0, 0].into()),
        config.server.port,
    );
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ScheduleConfig, ServerConfig};
    use chrono_tz::UTC;

    fn cfg(origin: Option<&str>) -> Config {
        Config {
            server: ServerConfig {
                host: "0.0.0.0".into(),
                port: 3001,
                cors_allowed_origin: origin.map(|s| s.to_string()),
            },
            database_url: "sqlite::memory:".into(),
            tmdb_api_key: "k".into(),
            slack_webhook_url: None,
            schedule: ScheduleConfig {
                resync_cron: "0 0 6 * * *".into(),
                notification_check_interval_minutes: 60,
            },
            timezone: UTC,
        }
    }

    #[test]
    fn cors_permissive_when_origin_is_wildcard() {
        // We can't introspect a CorsLayer's internals; just exercise the branch
        // and confirm it constructs without panicking.
        let _ = build_cors_layer(&cfg(Some("*")));
    }

    #[test]
    fn cors_specific_origin_constructs() {
        let _ = build_cors_layer(&cfg(Some("https://example.com")));
    }

    #[test]
    fn cors_default_origin_constructs() {
        let _ = build_cors_layer(&cfg(None));
    }

    #[test]
    #[should_panic]
    fn cors_invalid_origin_panics() {
        // The current impl explicitly expects a parsable origin.
        let _ = build_cors_layer(&cfg(Some("\u{0}invalid")));
    }
}
