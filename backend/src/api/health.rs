use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub database: bool,
    pub notifiers: Vec<String>,
}

pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let db_ok = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").fetch_one(&state.pool),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false);

    Json(HealthResponse {
        status: if db_ok { "ok" } else { "degraded" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database: db_ok,
        notifiers: state
            .notifier
            .channel_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
    })
}
