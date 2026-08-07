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
                    let names: Vec<String> =
                        where_to_watch.iter().map(|p| escape_mrkdwn(p)).collect();
                    format!(" — available on {}", names.join(", "))
                };
                format!(
                    "📺 *{}* — S{:02}E{:02} \"{}\" airs today{}",
                    escape_mrkdwn(show_name),
                    season,
                    episode,
                    escape_mrkdwn(title),
                    providers
                )
            }
            NotificationEvent::Test { message } => {
                format!("🔔 Showrunner test: {}", escape_mrkdwn(message))
            }
        }
    }
}

/// Escape text for Slack's mrkdwn so TMDB-sourced values can't inject markup.
///
/// Every field interpolated into a message here originates upstream (show
/// names, episode titles and provider names all come from TMDB, which is
/// community-editable), and `serde_json` escapes JSON, not mrkdwn — a
/// sanitizer/sink scope mismatch. Two classes matter:
///
/// - `&`, `<` and `>` are Slack's three documented HTML-escapes. Escaping `<`
///   is what defuses `<url|label>`, the link syntax that lets an attacker
///   choose both the destination and the anchor text a reader sees.
/// - `*`, `_`, `~` and `` ` `` are formatting metacharacters. They can't forge a
///   link, but they let a payload close the `*…*` span this formatter opens and
///   restyle the rest of the message. Slack has no escape for them, so they're
///   replaced with visually-equivalent Unicode look-alikes that carry no markup
///   meaning.
///
/// Escaping at this single formatter rather than per call site covers every
/// field, including any added later.
fn escape_mrkdwn(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '*' => out.push('\u{2217}'), // ∗ asterisk operator
            '_' => out.push('\u{2017}'), // ‗ double low line
            '~' => out.push('\u{223C}'), // ∼ tilde operator
            '`' => out.push('\u{2018}'), // ‘ left single quote
            _ => out.push(ch),
        }
    }
    out
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
        let n = SlackNotifier::new("http://127.0.0.1:1/webhook-secret-token".into());
        let err = n
            .send(&NotificationEvent::Test {
                message: "x".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Http(_)));
        // The webhook URL IS the Slack secret; it must never survive into the
        // error's string form, which handlers surface to clients and logs.
        let rendered = err.to_string();
        assert!(
            !rendered.contains("webhook-secret-token"),
            "webhook URL leaked into error: {rendered}"
        );
        assert!(
            !rendered.contains("127.0.0.1"),
            "webhook host leaked into error: {rendered}"
        );
    }

    // ===== VULN-007 (CWE-74) — mrkdwn / link injection from TMDB text =====

    const EVIL_SHOW_NAME: &str =
        "Severance <https://evil.example/slack-oauth|Click here to re-authorize Showrunner>";
    const EVIL_PROVIDER: &str = "<https://evil.example/pay|Netflix — billing update required>";

    fn episode_event(show_name: &str, title: &str, providers: Vec<String>) -> NotificationEvent {
        NotificationEvent::EpisodeAiringToday {
            show_name: show_name.into(),
            season: 2,
            episode: 1,
            title: title.into(),
            where_to_watch: providers,
        }
    }

    #[test]
    fn show_name_cannot_inject_a_slack_link() {
        let text =
            SlackNotifier::format(&episode_event(EVIL_SHOW_NAME, "Hello, Ms. Cobel", vec![]));
        assert!(
            !text.contains("<https://evil.example/slack-oauth|"),
            "attacker-controlled show name rendered as a live mrkdwn link: {text}"
        );
        // The visible text must survive — this is escaping, not stripping.
        assert!(text.contains("Severance"));
    }

    #[test]
    fn episode_title_cannot_inject_a_slack_link() {
        let text = SlackNotifier::format(&episode_event(
            "Severance",
            "<https://evil.example/x|Click me>",
            vec![],
        ));
        assert!(
            !text.contains("<https://evil.example/x|"),
            "attacker-controlled episode title rendered as a live mrkdwn link: {text}"
        );
    }

    #[test]
    fn provider_name_cannot_inject_a_slack_link() {
        let text = SlackNotifier::format(&episode_event(
            "Severance",
            "Hello, Ms. Cobel",
            vec![EVIL_PROVIDER.to_string()],
        ));
        assert!(
            !text.contains("<https://evil.example/pay|"),
            "attacker-controlled provider name rendered as a live mrkdwn link: {text}"
        );
    }

    /// The payload must not close the `*…*` bold span the formatter opens.
    #[test]
    fn show_name_cannot_break_out_of_the_bold_span() {
        let text = SlackNotifier::format(&episode_event(
            "Real* _totally legit_ *Show",
            "Pilot",
            vec![],
        ));
        assert_eq!(
            text.matches('*').count(),
            2,
            "attacker-controlled name introduced extra mrkdwn formatting: {text}"
        );
    }

    #[test]
    fn ampersand_is_html_escaped_per_slack_rules() {
        let text = SlackNotifier::format(&episode_event("Rick & Morty", "Pilot", vec![]));
        assert!(
            text.contains("Rick &amp; Morty"),
            "bare ampersand was not escaped per Slack's rules: {text}"
        );
    }

    #[test]
    fn test_event_message_is_also_escaped() {
        let text = SlackNotifier::format(&NotificationEvent::Test {
            message: "<https://evil.example|hi>".into(),
        });
        assert!(!text.contains("<https://evil.example|"));
    }

    /// The escaper must be a pure pass-through for text with no metacharacters.
    #[test]
    fn escape_mrkdwn_leaves_ordinary_text_alone() {
        assert_eq!(escape_mrkdwn("The Bear — Children"), "The Bear — Children");
    }
}
