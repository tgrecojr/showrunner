use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::error::{AppError, Result};
use crate::notifications::{NotificationEvent, Notifier};

const SLACK_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SlackNotifier {
    http: Client,
    webhook_url: String,
}

impl SlackNotifier {
    pub fn new(webhook_url: String) -> Self {
        let http = Client::builder()
            .timeout(SLACK_HTTP_TIMEOUT)
            .build()
            .expect("reqwest client with static config should build");
        Self { http, webhook_url }
    }

    fn format(event: &NotificationEvent) -> String {
        match event {
            NotificationEvent::EpisodeAiringToday {
                show_name,
                season,
                episode,
                title,
                where_to_watch,
            } => {
                let providers = if where_to_watch.is_empty() {
                    String::new()
                } else {
                    format!(" — available on {}", where_to_watch.join(", "))
                };
                format!(
                    "📺 *{}* — S{:02}E{:02} \"{}\" airs today{}",
                    show_name, season, episode, title, providers
                )
            }
            NotificationEvent::Test { message } => format!("🔔 Showrunner test: {}", message),
        }
    }
}

#[async_trait]
impl Notifier for SlackNotifier {
    fn name(&self) -> &str {
        "slack"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        let body = json!({ "text": Self::format(event) });
        let resp = self.http.post(&self.webhook_url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Upstream(format!(
                "Slack webhook returned {}",
                resp.status()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn name_is_slack() {
        let n = SlackNotifier::new("https://hooks.example/x".into());
        assert_eq!(n.name(), "slack");
    }

    #[test]
    fn format_episode_includes_show_season_episode_and_providers() {
        let event = NotificationEvent::EpisodeAiringToday {
            show_name: "The Bear".into(),
            season: 3,
            episode: 5,
            title: "Children".into(),
            where_to_watch: vec!["Hulu".into(), "FX".into()],
        };
        let s = SlackNotifier::format(&event);
        assert!(s.contains("The Bear"));
        assert!(s.contains("S03E05"));
        assert!(s.contains("Children"));
        assert!(s.contains("Hulu"));
        assert!(s.contains("FX"));
    }

    #[test]
    fn format_episode_omits_providers_segment_when_empty() {
        let event = NotificationEvent::EpisodeAiringToday {
            show_name: "X".into(),
            season: 1,
            episode: 1,
            title: "Pilot".into(),
            where_to_watch: vec![],
        };
        let s = SlackNotifier::format(&event);
        assert!(!s.contains("available on"));
    }

    #[test]
    fn format_test_event() {
        let s = SlackNotifier::format(&NotificationEvent::Test {
            message: "hi".into(),
        });
        assert!(s.contains("test"));
        assert!(s.contains("hi"));
    }

    #[tokio::test]
    async fn send_posts_to_webhook_and_returns_ok_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/hook"))
            .and(body_partial_json(
                serde_json::json!({"text": "🔔 Showrunner test: ping"}),
            ))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let n = SlackNotifier::new(format!("{}/hook", server.uri()));
        n.send(&NotificationEvent::Test {
            message: "ping".into(),
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn send_returns_upstream_on_non_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/hook"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let n = SlackNotifier::new(format!("{}/hook", server.uri()));
        let err = n
            .send(&NotificationEvent::Test {
                message: "x".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
        assert!(err.to_string().contains("500"));
    }

    #[tokio::test]
    async fn send_returns_http_error_when_unreachable() {
        // Reserved TEST-NET-1 IP — connections fail fast.
        let n = SlackNotifier::new("http://127.0.0.1:1/nope".into());
        let err = n
            .send(&NotificationEvent::Test {
                message: "x".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Http(_)));
    }
}
