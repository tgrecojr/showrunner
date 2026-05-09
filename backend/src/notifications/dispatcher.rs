use crate::error::{AppError, Result};
use crate::notifications::{NotificationEvent, Notifier};

pub struct NotificationDispatcher {
    notifiers: Vec<Box<dyn Notifier>>,
}

impl NotificationDispatcher {
    pub fn new(notifiers: Vec<Box<dyn Notifier>>) -> Self {
        Self { notifiers }
    }

    pub fn channel_names(&self) -> Vec<String> {
        self.notifiers
            .iter()
            .map(|n| n.name().to_string())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.notifiers.is_empty()
    }

    /// Send to every configured notifier; returns per-channel results.
    pub async fn dispatch(&self, event: &NotificationEvent) -> Vec<(String, Result<()>)> {
        let mut results = Vec::with_capacity(self.notifiers.len());
        for notifier in &self.notifiers {
            let name = notifier.name().to_string();
            let res = notifier.send(event).await;
            if let Err(e) = &res {
                tracing::warn!(channel = %name, error = %e, "notifier failed");
            }
            results.push((name, res));
        }
        results
    }

    /// Send to a single named notifier. Returns NotFound if no such channel.
    pub async fn dispatch_to(&self, channel: &str, event: &NotificationEvent) -> Result<()> {
        let notifier = self
            .notifiers
            .iter()
            .find(|n| n.name() == channel)
            .ok_or_else(|| AppError::NotFound(format!("notifier '{}' not configured", channel)))?;
        notifier.send(event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct StubNotifier {
        name: &'static str,
        calls: Arc<AtomicUsize>,
        fail: bool,
    }

    impl StubNotifier {
        fn new(name: &'static str, fail: bool) -> (Self, Arc<AtomicUsize>) {
            let calls = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    name,
                    calls: calls.clone(),
                    fail,
                },
                calls,
            )
        }
    }

    #[async_trait]
    impl Notifier for StubNotifier {
        fn name(&self) -> &str {
            self.name
        }
        async fn send(&self, _event: &NotificationEvent) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(AppError::Upstream("stubbed".into()))
            } else {
                Ok(())
            }
        }
    }

    fn test_event() -> NotificationEvent {
        NotificationEvent::Test {
            message: "hi".into(),
        }
    }

    #[test]
    fn empty_dispatcher_reports_no_channels() {
        let d = NotificationDispatcher::new(Vec::new());
        assert!(d.is_empty());
        assert!(d.channel_names().is_empty());
    }

    #[test]
    fn channel_names_lists_all_configured() {
        let (a, _) = StubNotifier::new("a", false);
        let (b, _) = StubNotifier::new("b", false);
        let d = NotificationDispatcher::new(vec![Box::new(a), Box::new(b)]);
        assert!(!d.is_empty());
        assert_eq!(d.channel_names(), vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn dispatch_calls_every_notifier_and_collects_results() {
        let (ok_n, ok_calls) = StubNotifier::new("ok", false);
        let (bad_n, bad_calls) = StubNotifier::new("bad", true);
        let d = NotificationDispatcher::new(vec![Box::new(ok_n), Box::new(bad_n)]);

        let results = d.dispatch(&test_event()).await;
        assert_eq!(ok_calls.load(Ordering::SeqCst), 1);
        assert_eq!(bad_calls.load(Ordering::SeqCst), 1);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "ok");
        assert!(results[0].1.is_ok());
        assert_eq!(results[1].0, "bad");
        assert!(results[1].1.is_err());
    }

    #[tokio::test]
    async fn dispatch_to_routes_to_named_channel() {
        let (a, a_calls) = StubNotifier::new("a", false);
        let (b, b_calls) = StubNotifier::new("b", false);
        let d = NotificationDispatcher::new(vec![Box::new(a), Box::new(b)]);

        d.dispatch_to("b", &test_event()).await.unwrap();
        assert_eq!(a_calls.load(Ordering::SeqCst), 0);
        assert_eq!(b_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_to_unknown_channel_returns_not_found() {
        let (a, _) = StubNotifier::new("a", false);
        let d = NotificationDispatcher::new(vec![Box::new(a)]);
        let err = d.dispatch_to("missing", &test_event()).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn dispatch_to_propagates_notifier_error() {
        let (bad, _) = StubNotifier::new("bad", true);
        let d = NotificationDispatcher::new(vec![Box::new(bad)]);
        let err = d.dispatch_to("bad", &test_event()).await.unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }
}
