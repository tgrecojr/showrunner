use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::db::queries::{self, NotifiableEpisode};
use crate::error::Result;
use crate::notifications::dispatcher::NotificationDispatcher;
use crate::notifications::NotificationEvent;
use crate::state::today_in;

/// Check for episodes airing today and dispatch notifications for any
/// (episode × channel) combinations not yet logged. Failures don't log,
/// so they retry on the next tick.
pub async fn check_and_notify(
    pool: &SqlitePool,
    dispatcher: &NotificationDispatcher,
    tz: Tz,
) -> Result<usize> {
    if dispatcher.is_empty() {
        return Ok(0);
    }

    let today = today_in(tz);
    let episodes = queries::list_episodes_airing_on(pool, &today).await?;
    if episodes.is_empty() {
        return Ok(0);
    }

    let channels = dispatcher.channel_names();
    let mut sent = 0_usize;

    for ep in &episodes {
        let event = build_event(ep);
        for channel in &channels {
            let already = queries::was_notified(
                pool,
                ep.show_tmdb_id,
                ep.season_number,
                ep.episode_number,
                channel,
            )
            .await?;
            if already {
                continue;
            }
            match dispatcher.dispatch_to(channel, &event).await {
                Ok(()) => {
                    queries::log_notification(
                        pool,
                        ep.show_tmdb_id,
                        ep.season_number,
                        ep.episode_number,
                        channel,
                    )
                    .await?;
                    sent += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        channel = %channel,
                        show = %ep.show_name,
                        season = ep.season_number,
                        episode = ep.episode_number,
                        error = %e,
                        "notification failed; will retry next tick"
                    );
                }
            }
        }
    }

    Ok(sent)
}

fn build_event(ep: &NotifiableEpisode) -> NotificationEvent {
    NotificationEvent::EpisodeAiringToday {
        show_name: ep.show_name.clone(),
        season: ep.season_number as u16,
        episode: ep.episode_number as u16,
        title: ep.episode_name.clone().unwrap_or_default(),
        where_to_watch: ep.watch_providers.clone(),
    }
}
