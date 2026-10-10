use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub database: bool,
}

/// `GET /api/v1/health`: 200 `ok` when `SELECT 1` answers within 2 s, else
/// 503 `degraded`. The status code is what container and proxy checks key
/// on; the body is for humans.
// @spec APP-HEALTH-001, APP-HEALTH-002
pub async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let db_ok = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").fetch_one(&state.pool),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false);

    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(HealthResponse {
            status: if db_ok { "ok" } else { "degraded" }.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            database: db_ok,
        }),
    )
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// The binary's own health probe, run by the image's HEALTHCHECK: one GET of
/// `/api/v1/health` over loopback. True only on HTTP 200; a refused
/// connection, a timeout, or any other status is false. Deliberately reads
/// nothing beyond the host and port so it cannot fail for a reason unrelated
/// to the server.
// @spec APP-HEALTH-003
pub async fn probe(host: &str, port: u16) -> bool {
    let host = match host.parse::<std::net::IpAddr>() {
        Ok(ip) if ip.is_unspecified() => {
            if ip.is_ipv4() {
                "127.0.0.1".to_string()
            } else {
                "[::1]".to_string()
            }
        }
        Ok(std::net::IpAddr::V6(v6)) => format!("[{}]", v6),
        _ => host.to_string(),
    };
    let url = format!("http://{}:{}/api/v1/health", host, port);
    let client = match reqwest::Client::builder().timeout(PROBE_TIMEOUT).build() {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client.get(&url).send().await {
        Ok(resp) => resp.status() == reqwest::StatusCode::OK,
        Err(_) => false,
    }
}

/// `probe` with the host and port read from the environment the way the
/// server reads them (`SERVER_HOST` default `0.0.0.0`, `SERVER_PORT` default
/// 3001). An unparsable port falls back to the default.
// @spec APP-HEALTH-003
pub async fn probe_from_env() -> bool {
    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);
    probe(&host, port).await
}
