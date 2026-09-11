use serde::Serialize;
use sqlx::FromRow;

/// One row of the append-only `watch_log` table.
#[derive(Debug, Clone, FromRow)]
pub struct WatchLogRow {
    pub id: i64,
    pub occurred_at: String,
    pub media_type: String,
    pub action: String,
    pub scope: String,
    pub tmdb_id: i64,
    pub title: String,
    pub poster_path: Option<String>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub episode_name: Option<String>,
    pub episode_count: i64,
}

/// Wire shape of a history entry.
#[derive(Debug, Serialize)]
pub struct WatchLogEntry {
    pub id: i64,
    pub occurred_at: String,
    pub media_type: String,
    pub action: String,
    pub scope: String,
    pub tmdb_id: i64,
    pub title: String,
    pub poster_url: Option<String>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub episode_name: Option<String>,
    pub episode_count: i64,
}

#[derive(Debug, Serialize)]
pub struct WatchLogPage {
    pub entries: Vec<WatchLogEntry>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

/// What a mutation records. Built by the query layer at the moment the state
/// change is applied, inside the same transaction.
#[derive(Debug, Clone)]
pub struct NewWatchLogEntry<'a> {
    pub media_type: &'a str,
    pub action: &'a str,
    pub scope: &'a str,
    pub tmdb_id: i64,
    pub title: &'a str,
    pub poster_path: Option<&'a str>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub episode_name: Option<&'a str>,
    pub episode_count: i64,
}

pub fn action_str(watched: bool) -> &'static str {
    if watched {
        "watched"
    } else {
        "unwatched"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_str_maps_bool() {
        assert_eq!(action_str(true), "watched");
        assert_eq!(action_str(false), "unwatched");
    }
}
