use serde::{Deserialize, Serialize};
use sqlx::FromRow;

const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w185";
const BACKDROP_BASE: &str = "https://image.tmdb.org/t/p/w780";

pub fn poster_url(path: Option<&str>) -> Option<String> {
    path.filter(|p| !p.is_empty())
        .map(|p| format!("{}{}", POSTER_BASE, p))
}

pub fn backdrop_url(path: Option<&str>) -> Option<String> {
    path.filter(|p| !p.is_empty())
        .map(|p| format!("{}{}", BACKDROP_BASE, p))
}

// === Database rows ===

// Row structs mirror their tables for sqlx::FromRow; some columns aren't
// surfaced yet but we want to keep the rows complete for forward use
// (e.g. per-show notify toggle UI, watched_at history view).
#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct ShowRow {
    pub tmdb_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub status: Option<String>,
    pub first_air_date: Option<String>,
    pub last_air_date: Option<String>,
    pub in_production: i64,
    pub watch_providers_json: Option<String>,
    pub notify_new_episodes: i64,
    pub added_at: String,
    pub last_synced_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct SeasonRow {
    pub show_tmdb_id: i64,
    pub season_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub episode_count: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct EpisodeRow {
    pub show_tmdb_id: i64,
    pub season_number: i64,
    pub episode_number: i64,
    pub tmdb_id: Option<i64>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub runtime: Option<i64>,
    pub watched: i64,
    pub watched_at: Option<String>,
}

// === API response shapes ===

#[derive(Debug, Serialize)]
pub struct WatchlistItem {
    pub tmdb_id: i64,
    pub name: String,
    pub poster_url: Option<String>,
    pub status: Option<String>,
    pub in_production: bool,
    pub watched_count: i64,
    pub aired_count: i64,
    pub total_count: i64,
    pub next_episode_air_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShowDetail {
    pub tmdb_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub status: Option<String>,
    pub in_production: bool,
    pub first_air_date: Option<String>,
    pub last_air_date: Option<String>,
    pub watch_providers: Vec<String>,
    pub seasons: Vec<SeasonDetail>,
}

#[derive(Debug, Serialize)]
pub struct SeasonDetail {
    pub season_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub episode_count: i64,
    pub watched_count: i64,
    pub episodes: Vec<EpisodeDetail>,
}

#[derive(Debug, Serialize)]
pub struct EpisodeDetail {
    pub episode_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub runtime: Option<i64>,
    pub watched: bool,
}

#[derive(Debug, Serialize)]
pub struct UpNextItem {
    pub show_tmdb_id: i64,
    pub show_name: String,
    pub poster_url: Option<String>,
    pub season_number: i64,
    pub episode_number: i64,
    pub episode_name: Option<String>,
    pub episode_overview: Option<String>,
    pub air_date: String,
    pub remaining: i64,
}

#[derive(Debug, Serialize)]
pub struct CalendarEpisode {
    pub show_tmdb_id: i64,
    pub show_name: String,
    pub poster_url: Option<String>,
    pub season_number: i64,
    pub episode_number: i64,
    pub episode_name: Option<String>,
    pub air_date: String,
    pub watched: bool,
}

// === Request shape ===

#[derive(Debug, Deserialize)]
pub struct AddShowRequest {
    pub tmdb_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poster_url_prepends_base_when_path_present() {
        assert_eq!(
            poster_url(Some("/abc.jpg")),
            Some("https://image.tmdb.org/t/p/w185/abc.jpg".to_string()),
        );
    }

    #[test]
    fn poster_url_treats_empty_string_as_none() {
        assert_eq!(poster_url(Some("")), None);
    }

    #[test]
    fn poster_url_returns_none_for_none() {
        assert_eq!(poster_url(None), None);
    }

    #[test]
    fn backdrop_url_uses_w780_base() {
        assert_eq!(
            backdrop_url(Some("/x.jpg")),
            Some("https://image.tmdb.org/t/p/w780/x.jpg".to_string()),
        );
        assert_eq!(backdrop_url(Some("")), None);
        assert_eq!(backdrop_url(None), None);
    }

    #[test]
    fn add_show_request_deserializes() {
        let req: AddShowRequest = serde_json::from_str(r#"{"tmdb_id":1399}"#).unwrap();
        assert_eq!(req.tmdb_id, 1399);
    }
}
