pub mod dispatcher;
pub mod slack;

use async_trait::async_trait;
use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub enum NotificationEvent {
    EpisodeAiringToday {
        show_name: String,
        season: i64,
        episode: i64,
        title: String,
        where_to_watch: Vec<String>,
    },
    Test {
        message: String,
    },
}

#[async_trait]
pub trait Notifier: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, event: &NotificationEvent) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episode_event_serializes_fields() {
        let event = NotificationEvent::EpisodeAiringToday {
            show_name: "S".into(),
            season: 1,
            episode: 2,
            title: "t".into(),
            where_to_watch: vec!["Hulu".into()],
        };
        let v = serde_json::to_value(&event).unwrap();
        let inner = &v["EpisodeAiringToday"];
        assert_eq!(inner["show_name"], "S");
        assert_eq!(inner["season"], 1);
        assert_eq!(inner["episode"], 2);
        assert_eq!(inner["title"], "t");
        assert_eq!(inner["where_to_watch"][0], "Hulu");
    }

    #[test]
    fn test_event_serializes_message() {
        let event = NotificationEvent::Test {
            message: "ping".into(),
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["Test"]["message"], "ping");
    }

    #[test]
    fn event_clone_preserves_data() {
        let original = NotificationEvent::Test {
            message: "x".into(),
        };
        let clone = original.clone();
        match clone {
            NotificationEvent::Test { message } => assert_eq!(message, "x"),
            _ => panic!("wrong variant"),
        }
    }
}
