pub mod api;
pub mod config;
pub mod datasources;
pub mod db;
pub mod error;
pub mod logic;
pub mod models;
pub mod scheduler;
pub mod state;

use crate::config::Config;
use crate::datasources::tmdb::TmdbClient;
use crate::db::pool::create_pool;
use crate::state::AppState;
use axum::error_handling::HandleErrorLayer;
use axum::extract::Request;
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{BoxError, Router};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tower::limit::ConcurrencyLimitLayer;
use tower::load_shed::LoadShedLayer;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tracing_subscriber::EnvFilter;

const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Ceiling on API requests in flight at once. Sized generously against the
/// SQLite pool (5 connections by default) — this bounds a flood, it isn't a
/// throughput target for a single-user homelab app.
const MAX_INFLIGHT_REQUESTS: usize = 64;

// The SPA loads its own JS/CSS bundle and poster images from TMDB's image CDN;
// nothing else. Scripts are locked to same-origin (no inline/CDN scripts in the
// production build), framing is denied, and image sources are pinned to TMDB.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; \
img-src 'self' https://image.tmdb.org data:; \
script-src 'self'; \
style-src 'self' 'unsafe-inline'; \
connect-src 'self'; \
frame-ancestors 'none'; \
base-uri 'self'; \
form-action 'self'; \
object-src 'none'";

/// Wrap the fully-assembled app (API routes + static fallback) with security
/// response headers. Applied last so it also covers the static HTML/JS, which
/// is where the CSP and anti-framing headers actually matter.
pub fn with_security_headers(app: Router) -> Router {
    app.layer(SetResponseHeaderLayer::if_not_present(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    ))
}

/// Reject state-changing requests that don't carry `Content-Type:
/// application/json`.
///
/// A cross-origin HTML form can only emit `application/x-www-form-urlencoded`,
/// `multipart/form-data` or `text/plain` — all CORS *simple* content types, so
/// the browser sends them with no preflight and `CorsLayer` never gets a say.
/// Requiring `application/json` makes the request non-simple: a forged form
/// submission is rejected here, and a scripted cross-origin `fetch` has to pass
/// a preflight first. Routes taking `Json<T>` already get this incidentally;
/// applying it at the router extends the same protection to body-less handlers
/// and to any route added later.
///
/// Residual: this proves a request did not originate from a cross-origin HTML
/// form. It does not authenticate the caller — any non-browser client (curl, a
/// script) can still set the header and reach these routes. Closing that is the
/// deferred auth/authz work, not this layer's job.
async fn require_json_content_type(req: Request, next: Next) -> Response {
    let guarded = matches!(*req.method(), Method::POST | Method::PUT | Method::PATCH);
    if guarded && !is_json_content_type(req.headers().get(header::CONTENT_TYPE)) {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    next.run(req).await
}

/// `application/json`, or any `application/<subtype>+json`, ignoring parameters
/// such as `; charset=utf-8`. Matching axum's own `Json` extractor.
fn is_json_content_type(value: Option<&HeaderValue>) -> bool {
    let Some(raw) = value.and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let essence = raw.split(';').next().unwrap_or("").trim();
    let Some((ty, subtype)) = essence.split_once('/') else {
        return false;
    };
    ty.eq_ignore_ascii_case("application")
        && (subtype.eq_ignore_ascii_case("json")
            || subtype
                .rsplit_once('+')
                .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case("json")))
}

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
        .route(
            "/api/v1/movies",
            get(api::movies::list_movies).post(api::movies::add_movie),
        )
        .route(
            "/api/v1/movies/{tmdb_id}",
            get(api::movies::get_movie_detail).delete(api::movies::delete_movie),
        )
        .route("/api/v1/calendar", get(api::calendar::get_calendar))
        .route("/api/v1/up-next", get(api::up_next::list_up_next))
        .route("/api/v1/sync", post(api::sync::manual_sync))
        .layer(middleware::from_fn(require_json_content_type))
        // RequestBodyLimit and Timeout below bound request SIZE and TIME.
        // Neither bounds how many requests are in flight at once, so add an
        // explicit ceiling: LoadShed turns the queue ConcurrencyLimit would
        // otherwise build into an immediate 503, so an unauthenticated flood
        // sheds instead of accumulating unbounded pending work. HandleError
        // maps the shed back to a response, since axum needs the stack to be
        // infallible.
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|err: BoxError| async move {
                    if err.is::<tower::load_shed::error::Overloaded>() {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                }))
                .layer(LoadShedLayer::new())
                .layer(ConcurrencyLimitLayer::new(MAX_INFLIGHT_REQUESTS)),
        )
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            API_REQUEST_TIMEOUT,
        ))
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
            // Deliberately built by hand rather than with the tower-http
            // catch-all constructor: that one also sets allow_methods(Any) and
            // expose_headers(Any), a strictly broader grant than the
            // named-origin branch below. Build the same layer that branch does,
            // differing only in which origins are accepted.
            // Note the absence of `allow_credentials(true)` — pairing it with a
            // wildcard origin would turn this into a credentialed cross-origin
            // read, and the CORS spec forbids the combination outright.
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(methods)
                .allow_headers(tower_http::cors::Any)
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

    let state = AppState::new(pool, tmdb, config.timezone);

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

    let app = with_security_headers(app);

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
            schedule: ScheduleConfig {
                resync_cron: "0 0 6 * * *".into(),
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

    #[tokio::test]
    async fn security_headers_are_applied_to_responses() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let app = with_security_headers(
            Router::new().route("/ping", axum::routing::get(|| async { "ok" })),
        );
        let resp = app
            .oneshot(Request::builder().uri("/ping").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let headers = resp.headers();
        assert!(headers
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'"));
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
    }
}
