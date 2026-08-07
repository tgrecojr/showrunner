use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::notifications::NotificationEvent;
use crate::state::{AppState, NOTIFY_TEST_MIN_INTERVAL};

// Fixed text dispatched by `POST /notifications/test`. The endpoint takes no
// body — accepting a user-controlled message would let any unauthenticated
// caller broadcast arbitrary content to every configured channel.
const TEST_MESSAGE: &str = "Hello from Showrunner.";

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

pub async fn test_notification(State(state): State<AppState>) -> Result<Json<TestResponse>> {
    // Unauthenticated and outbound-spending: without a floor on how often it can
    // fire, this endpoint floods the operator's configured webhooks.
    if !state.notify_test_gate.try_acquire(NOTIFY_TEST_MIN_INTERVAL) {
        return Err(AppError::TooManyRequests(format!(
            "a test notification was sent less than {}s ago; try again shortly",
            NOTIFY_TEST_MIN_INTERVAL.as_secs()
        )));
    }

    let event = NotificationEvent::Test {
        message: TEST_MESSAGE.to_string(),
    };

    let dispatched = state.notifier.dispatch(&event).await;
    let results = dispatched
        .into_iter()
        .map(|(channel, res)| ChannelResult {
            channel,
            ok: res.is_ok(),
            // Client-safe: serialized into the response body, so it must not
            // carry the webhook URL or other internal detail.
            error: res.err().map(|e| e.client_message()),
        })
        .collect();

    Ok(Json(TestResponse { results }))
}
