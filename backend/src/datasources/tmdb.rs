use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::error::{AppError, Result};

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct TmdbClient {
    http: Client,
    api_key: String,
    base_url: String,
}

impl TmdbClient {
    pub fn new(api_key: String) -> Self {
        Self::with_base_url(api_key, TMDB_BASE_URL.to_string())
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let http = Client::builder()
            .timeout(TMDB_HTTP_TIMEOUT)
            .build()
            .expect("reqwest client with static config should build");
        Self {
            http,
            api_key,
            base_url,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub async fn get_show(&self, tmdb_id: i64) -> Result<TmdbShow> {
        let url = format!("{}/tv/{}", self.base_url, tmdb_id);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("api_key", self.api_key.as_str()),
                ("append_to_response", "watch/providers"),
            ])
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "show {} not found on TMDB",
                tmdb_id
            )));
        }
        if !resp.status().is_success() {
            return Err(AppError::Upstream(format!(
                "TMDB returned {}",
                resp.status()
            )));
        }
        Ok(resp.json().await?)
    }

    pub async fn get_movie(&self, tmdb_id: i64) -> Result<TmdbMovie> {
        let url = format!("{}/movie/{}", self.base_url, tmdb_id);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("api_key", self.api_key.as_str()),
                ("append_to_response", "credits,watch/providers"),
            ])
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "movie {} not found on TMDB",
                tmdb_id
            )));
        }
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Upstream(
                "TMDB is rate-limiting requests right now. Please try again in a moment."
                    .to_string(),
            ));
        }
        if !resp.status().is_success() {
            return Err(AppError::Upstream(format!(
                "TMDB returned {}",
                resp.status()
            )));
        }
        Ok(resp.json().await?)
    }

    pub async fn get_season(&self, tmdb_id: i64, season_number: i64) -> Result<TmdbSeason> {
        let url = format!("{}/tv/{}/season/{}", self.base_url, tmdb_id, season_number);
        let resp = self
            .http
            .get(&url)
            .query(&[("api_key", self.api_key.as_str())])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::Upstream(format!(
                "TMDB season {} returned {}",
                season_number,
                resp.status()
            )));
        }
        Ok(resp.json().await?)
    }
}

// === TMDB response shapes (only fields we use) ===

#[derive(Debug, Deserialize)]
pub struct TmdbShow {
    pub id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub status: Option<String>,
    pub first_air_date: Option<String>,
    pub last_air_date: Option<String>,
    #[serde(default)]
    pub in_production: bool,
    #[serde(default)]
    pub seasons: Vec<TmdbSeasonSummary>,
    #[serde(rename = "watch/providers", default)]
    pub watch_providers: Option<TmdbWatchProvidersWrapper>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbSeasonSummary {
    pub season_number: i64,
}

#[derive(Debug, Deserialize)]
pub struct TmdbSeason {
    pub season_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    #[serde(default)]
    pub episodes: Vec<TmdbEpisode>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbEpisode {
    pub id: Option<i64>,
    pub episode_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub runtime: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbMovie {
    pub id: i64,
    // TMDB uses `title` for movies (not `name`).
    pub title: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i64>,
    #[serde(default)]
    pub credits: Option<TmdbCredits>,
    #[serde(rename = "watch/providers", default)]
    pub watch_providers: Option<TmdbWatchProvidersWrapper>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbCredits {
    #[serde(default)]
    pub cast: Vec<TmdbCastMember>,
    #[serde(default)]
    pub crew: Vec<TmdbCrewMember>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbCastMember {
    pub name: String,
    #[serde(default)]
    pub character: Option<String>,
    #[serde(default)]
    pub profile_path: Option<String>,
    #[serde(default)]
    pub order: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbCrewMember {
    pub name: String,
    pub job: String,
    #[serde(default)]
    pub department: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbWatchProvidersWrapper {
    #[serde(default)]
    pub results: std::collections::HashMap<String, TmdbWatchProvidersByRegion>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbWatchProvidersByRegion {
    #[serde(default)]
    pub flatrate: Vec<TmdbProvider>,
    #[serde(default)]
    pub free: Vec<TmdbProvider>,
    #[serde(default)]
    pub ads: Vec<TmdbProvider>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbProvider {
    pub provider_name: String,
}

impl TmdbShow {
    /// Extract a simple list of US streaming/free/ads provider names.
    pub fn us_providers(&self) -> Vec<String> {
        us_providers_from(self.watch_providers.as_ref())
    }
}

impl TmdbMovie {
    pub fn us_providers(&self) -> Vec<String> {
        us_providers_from(self.watch_providers.as_ref())
    }

    pub fn directors(&self) -> Vec<String> {
        let Some(credits) = &self.credits else {
            return Vec::new();
        };
        credits
            .crew
            .iter()
            .filter(|c| c.job == "Director")
            .map(|c| c.name.clone())
            .collect()
    }
}

fn us_providers_from(wrapper: Option<&TmdbWatchProvidersWrapper>) -> Vec<String> {
    let Some(wrapper) = wrapper else {
        return Vec::new();
    };
    let Some(region) = wrapper.results.get("US") else {
        return Vec::new();
    };
    let mut names: Vec<String> = region
        .flatrate
        .iter()
        .chain(region.free.iter())
        .chain(region.ads.iter())
        .map(|p| p.provider_name.clone())
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn show_with_region(json: serde_json::Value) -> TmdbShow {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn us_providers_returns_empty_when_no_wrapper() {
        let show = show_with_region(serde_json::json!({
            "id": 1, "name": "x"
        }));
        assert!(show.us_providers().is_empty());
    }

    #[test]
    fn us_providers_returns_empty_when_no_us_region() {
        let show = show_with_region(serde_json::json!({
            "id": 1, "name": "x",
            "watch/providers": { "results": { "GB": { "flatrate": [] } } }
        }));
        assert!(show.us_providers().is_empty());
    }

    #[test]
    fn us_providers_concatenates_flatrate_free_and_ads_sorted_unique() {
        let show = show_with_region(serde_json::json!({
            "id": 1, "name": "x",
            "watch/providers": {
                "results": {
                    "US": {
                        "flatrate": [{"provider_name": "Hulu"}, {"provider_name": "Hulu"}],
                        "free": [{"provider_name": "Tubi"}],
                        "ads": [{"provider_name": "Pluto TV"}]
                    }
                }
            }
        }));
        assert_eq!(
            show.us_providers(),
            vec![
                "Hulu".to_string(),
                "Pluto TV".to_string(),
                "Tubi".to_string()
            ]
        );
    }

    #[test]
    fn new_uses_real_tmdb_base_url() {
        let c = TmdbClient::new("k".into());
        assert_eq!(c.base_url(), TMDB_BASE_URL);
        assert_eq!(c.api_key(), "k");
    }

    #[test]
    fn with_base_url_overrides() {
        let c = TmdbClient::with_base_url("k".into(), "http://example".into());
        assert_eq!(c.base_url(), "http://example");
    }

    #[tokio::test]
    async fn get_show_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/42"))
            .and(query_param("api_key", "test_key"))
            .and(query_param("append_to_response", "watch/providers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 42,
                "name": "The Bear",
                "overview": "kitchen",
                "in_production": true,
                "seasons": [{"season_number": 0}, {"season_number": 1}],
                "watch/providers": {"results": {"US": {"flatrate": [{"provider_name": "Hulu"}]}}}
            })))
            .mount(&server)
            .await;

        let c = TmdbClient::with_base_url("test_key".into(), server.uri());
        let show = c.get_show(42).await.unwrap();
        assert_eq!(show.id, 42);
        assert_eq!(show.name, "The Bear");
        assert_eq!(show.seasons.len(), 2);
        assert!(show.in_production);
        assert_eq!(show.us_providers(), vec!["Hulu".to_string()]);
    }

    #[tokio::test]
    async fn get_show_404_maps_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/9"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let err = c.get_show(9).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn get_show_500_maps_to_upstream() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/9"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let err = c.get_show(9).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::Upstream(_)));
    }

    #[tokio::test]
    async fn get_season_parses_episodes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/42/season/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "season_number": 1,
                "name": "S1",
                "episodes": [
                    {"id": 100, "episode_number": 1, "name": "Pilot",
                     "air_date": "2024-01-01", "runtime": 30}
                ]
            })))
            .mount(&server)
            .await;

        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let season = c.get_season(42, 1).await.unwrap();
        assert_eq!(season.season_number, 1);
        assert_eq!(season.episodes.len(), 1);
        assert_eq!(season.episodes[0].name.as_deref(), Some("Pilot"));
        assert_eq!(season.episodes[0].runtime, Some(30));
    }

    #[tokio::test]
    async fn get_movie_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/27205"))
            .and(query_param("api_key", "test_key"))
            .and(query_param("append_to_response", "credits,watch/providers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 27205,
                "title": "Inception",
                "overview": "dreams",
                "poster_path": "/p.jpg",
                "backdrop_path": "/b.jpg",
                "release_date": "2010-07-16",
                "runtime": 148,
                "credits": {
                    "cast": [
                        {"name": "Leonardo DiCaprio", "character": "Cobb",
                         "profile_path": "/leo.jpg", "order": 0},
                        {"name": "Joseph Gordon-Levitt", "character": "Arthur",
                         "profile_path": null, "order": 1}
                    ],
                    "crew": [
                        {"name": "Christopher Nolan", "job": "Director",
                         "department": "Directing"},
                        {"name": "Hans Zimmer", "job": "Original Music Composer",
                         "department": "Sound"}
                    ]
                },
                "watch/providers": {
                    "results": {
                        "US": {"flatrate": [{"provider_name": "Netflix"}]}
                    }
                }
            })))
            .mount(&server)
            .await;

        let c = TmdbClient::with_base_url("test_key".into(), server.uri());
        let movie = c.get_movie(27205).await.unwrap();
        assert_eq!(movie.id, 27205);
        assert_eq!(movie.title, "Inception");
        assert_eq!(movie.runtime, Some(148));
        assert_eq!(movie.release_date.as_deref(), Some("2010-07-16"));
        let credits = movie.credits.as_ref().expect("credits present");
        assert_eq!(credits.cast.len(), 2);
        assert_eq!(credits.cast[0].character.as_deref(), Some("Cobb"));
        assert_eq!(movie.directors(), vec!["Christopher Nolan".to_string()]);
        assert_eq!(movie.us_providers(), vec!["Netflix".to_string()]);
    }

    #[tokio::test]
    async fn get_movie_429_maps_to_rate_limit_message() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/9"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let err = c.get_movie(9).await.unwrap_err();
        match err {
            crate::error::AppError::Upstream(msg) => {
                assert!(msg.contains("rate-limiting"), "unexpected msg: {msg}");
            }
            other => panic!("expected Upstream, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_movie_404_maps_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/9"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let err = c.get_movie(9).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn get_movie_500_maps_to_upstream() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/9"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let err = c.get_movie(9).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::Upstream(_)));
    }

    #[tokio::test]
    async fn get_season_non_2xx_maps_to_upstream() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/42/season/1"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let err = c.get_season(42, 1).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::Upstream(_)));
    }
}
