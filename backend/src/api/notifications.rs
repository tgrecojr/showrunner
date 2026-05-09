use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::notifications::NotificationEvent;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct TestRequest {
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChannelResult {
    pub channel: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestResponse {
    pub results: Vec<ChannelResult>,
}

pub async fn test_notification(
    State(state): State<AppState>,
    body: Option<Json<TestRequest>>,
) -> Result<Json<TestResponse>> {
    let req = body.map(|Json(b)| b).unwrap_or_default();
    let event = NotificationEvent::Test {
        message: req
            .message
            .unwrap_or_else(|| "Hello from Showrunner.".to_string()),
    };

    let dispatched = state.notifier.dispatch(&event).await;
    let results = dispatched
        .into_iter()
        .map(|(channel, res)| ChannelResult {
            channel,
            ok: res.is_ok(),
            error: res.err().map(|e| e.to_string()),
        })
        .collect();

    Ok(Json(TestResponse { results }))
}
