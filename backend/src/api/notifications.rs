use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::Result;
use crate::notifications::NotificationEvent;
use crate::state::AppState;

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
    let event = NotificationEvent::Test {
        message: TEST_MESSAGE.to_string(),
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
