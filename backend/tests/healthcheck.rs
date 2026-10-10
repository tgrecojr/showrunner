//! The binary's `--healthcheck` probe mode: a loopback GET of
//! `/api/v1/health` whose exit code the image's HEALTHCHECK keys on.

mod common;

use showrunner_backend::api::health::probe;
use showrunner_backend::build_api_router;
use std::net::SocketAddr;

use crate::common::*;

/// Serve the API router on an ephemeral loopback port; returns the port.
async fn serve(state: showrunner_backend::state::AppState) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, build_api_router(state))
            .await
            .unwrap();
    });
    addr.port()
}

// @spec APP-HEALTH-003
#[tokio::test]
async fn probe_succeeds_when_health_is_200() {
    let pool = test_pool().await;
    let port = serve(app_state(pool, "http://unused".into(), utc_tz())).await;
    assert!(probe("127.0.0.1", port).await);
}

// @spec APP-HEALTH-003
#[tokio::test]
async fn probe_fails_when_health_is_503() {
    let pool = test_pool().await;
    let port = serve(app_state(pool.clone(), "http://unused".into(), utc_tz())).await;
    pool.close().await;
    assert!(!probe("127.0.0.1", port).await);
}

// @spec APP-HEALTH-003
#[tokio::test]
async fn probe_fails_when_nothing_is_listening() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    assert!(!probe("127.0.0.1", port).await);
}

// @spec APP-HEALTH-003
#[tokio::test]
async fn probe_treats_unspecified_host_as_loopback() {
    let pool = test_pool().await;
    let port = serve(app_state(pool, "http://unused".into(), utc_tz())).await;
    assert!(probe("0.0.0.0", port).await);
}
