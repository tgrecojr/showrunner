use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::error::{AppError, Result};

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
/// Real TMDB responses are well under 1 MiB; this only guards against a
/// misbehaving or compromised upstream advertising a huge body. Bodies sent
/// without a `Content-Length` (chunked) aren't capped here, but the request
/// timeout still bounds them.
const MAX_TMDB_BODY_BYTES: u64 = 16 * 1024 * 1024;

/// Deserialize a response as JSON, rejecting an oversized advertised body
/// before buffering it into memory.
pub(crate) async fn json_within_cap<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T> {
    if let Some(len) = resp.content_length() {
        if len > MAX_TMDB_BODY_BYTES {
            return Err(AppError::Upstream(
                "TMDB response was unexpectedly large".to_string(),
            ));
        }
    }
    Ok(resp.json().await?)
}

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

    /// Send `GET {base}/{path}` with the API key and the given extra query
    /// parameters. The key never leaves this method.
    async fn get(&self, path: &str, params: &[(&str, &str)]) -> Result<reqwest::Response> {
        let url = format!("{}/{}", self.base_url, path);
        let mut query: Vec<(&str, &str)> = vec![("api_key", self.api_key.as_str())];
        query.extend_from_slice(params);
        Ok(self.http.get(&url).query(&query).send().await?)
    }

    // @spec TMDB-CLIENT-002, TMDB-ERR-001
    pub async fn get_show(&self, tmdb_id: i64) -> Result<TmdbShow> {
        let resp = self
            .get(
                &format!("tv/{}", tmdb_id),
                &[("append_to_response", "watch/providers")],
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "show {} not found on TMDB",
                tmdb_id
            )));
        }
        map_status(&resp, None)?;
        json_within_cap(resp).await
    }

    // @spec TMDB-CLIENT-003, TMDB-ERR-001
    pub async fn get_movie(&self, tmdb_id: i64) -> Result<TmdbMovie> {
        let resp = self
            .get(
                &format!("movie/{}", tmdb_id),
                &[("append_to_response", "credits,watch/providers")],
            )
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "movie {} not found on TMDB",
                tmdb_id
            )));
        }
        map_status(&resp, None)?;
        json_within_cap(resp).await
    }

    // @spec TMDB-CLIENT-004
    pub async fn get_season(&self, tmdb_id: i64, season_number: i64) -> Result<TmdbSeason> {
        let resp = self
            .get(&format!("tv/{}/season/{}", tmdb_id, season_number), &[])
            .await?;
        map_status(&resp, Some(season_number))?;
        json_within_cap(resp).await
    }

    /// TMDB `search/multi`: TV, movies, and persons in one ranked list.
    /// Callers filter by `media_type`.
    // @spec TMDB-CLIENT-007
    pub async fn search_multi(&self, query: &str) -> Result<TmdbSearchResponse> {
        let resp = self
            .get(
                "search/multi",
                &[("query", query), ("include_adult", "false")],
            )
            .await?;
        map_status(&resp, None)?;
        json_within_cap(resp).await
    }
}

pub const RATE_LIMIT_MESSAGE: &str =
    "TMDB is rate-limiting requests right now. Please try again in a moment.";
pub const UNAVAILABLE_MESSAGE: &str = "TMDB is unavailable right now. Please try again shortly.";

/// Turn a non-2xx TMDB response into the error every caller surfaces. 429 and
/// 5xx become fixed user-readable sentences; anything else keeps the raw
/// status, which is the useful part when (say) the key is rejected with 401.
/// Per-method 404 rules run before this.
// @spec TMDB-ERR-002, TMDB-ERR-003, TMDB-ERR-004
fn map_status(resp: &reqwest::Response, season_number: Option<i64>) -> Result<()> {
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AppError::Upstream(RATE_LIMIT_MESSAGE.to_string()));
    }
    if status.is_server_error() {
        return Err(AppError::Upstream(UNAVAILABLE_MESSAGE.to_string()));
    }
    Err(AppError::Upstream(match season_number {
        Some(n) => format!("TMDB season {} returned {}", n, status),
        None => format!("TMDB returned {}", status),
    }))
}

// === TMDB response shapes (only fields we use) ===

/// Raw shape from TMDB's /search/multi: each result carries a `media_type`
/// discriminator and TV/movie-specific fields. Persons are left for callers
/// to filter out.
#[derive(Debug, Deserialize)]
pub struct TmdbSearchResponse {
    pub results: Vec<TmdbMultiResult>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbMultiResult {
    pub id: i64,
    pub media_type: Option<String>,
    // TV uses `name` + `first_air_date`; movies use `title` + `release_date`.
    pub name: Option<String>,
    pub title: Option<String>,
    pub first_air_date: Option<String>,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

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
    #[serde(default)]
    pub networks: Vec<TmdbNetwork>,
    #[serde(rename = "watch/providers", default)]
    pub watch_providers: Option<TmdbWatchProvidersWrapper>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbNetwork {
    pub name: String,
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

    /// Network names (broadcast/cable channels and streaming originals alike),
    /// in TMDB's order, without blanks or duplicates.
    pub fn network_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for n in &self.networks {
            let name = n.name.trim();
            if !name.is_empty() && !names.iter().any(|existing| existing == name) {
                names.push(name.to_string());
            }
        }
        names
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
    fn network_names_keeps_order_and_drops_blanks_and_duplicates() {
        let show = show_with_region(serde_json::json!({
            "id": 1, "name": "X",
            "networks": [
                {"id": 6, "name": "NBC"},
                {"id": 0, "name": " "},
                {"id": 3353, "name": "Peacock"},
                {"id": 6, "name": "NBC"}
            ]
        }));
        assert_eq!(
            show.network_names(),
            vec!["NBC".to_string(), "Peacock".to_string()]
        );
    }

    #[test]
    fn network_names_is_empty_when_field_missing() {
        let show = show_with_region(serde_json::json!({"id": 1, "name": "X"}));
        assert!(show.network_names().is_empty());
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

    // @spec TMDB-ERR-001
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

    async fn upstream_message(err: crate::error::AppError) -> String {
        match err {
            crate::error::AppError::Upstream(msg) => msg,
            other => panic!("expected Upstream, got {other:?}"),
        }
    }

    async fn mock_status(server: &MockServer, p: &str, status: u16) {
        Mock::given(method("GET"))
            .and(path(p))
            .respond_with(ResponseTemplate::new(status))
            .mount(server)
            .await;
    }

    // @spec TMDB-ERR-003
    #[tokio::test]
    async fn get_show_500_maps_to_unavailable_message() {
        let server = MockServer::start().await;
        mock_status(&server, "/tv/9", 500).await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let msg = upstream_message(c.get_show(9).await.unwrap_err()).await;
        assert_eq!(
            msg,
            "TMDB is unavailable right now. Please try again shortly."
        );
    }

    // @spec TMDB-ERR-002
    #[tokio::test]
    async fn get_show_429_maps_to_rate_limit_message() {
        let server = MockServer::start().await;
        mock_status(&server, "/tv/9", 429).await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let msg = upstream_message(c.get_show(9).await.unwrap_err()).await;
        assert_eq!(
            msg,
            "TMDB is rate-limiting requests right now. Please try again in a moment."
        );
    }

    // @spec TMDB-ERR-004
    #[tokio::test]
    async fn get_show_401_keeps_raw_status_text() {
        let server = MockServer::start().await;
        mock_status(&server, "/tv/9", 401).await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let msg = upstream_message(c.get_show(9).await.unwrap_err()).await;
        assert_eq!(msg, "TMDB returned 401 Unauthorized");
    }

    // @spec TMDB-ERR-002, TMDB-ERR-003
    #[tokio::test]
    async fn get_season_429_and_5xx_map_to_friendly_messages() {
        let server = MockServer::start().await;
        mock_status(&server, "/tv/42/season/1", 429).await;
        mock_status(&server, "/tv/42/season/2", 502).await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let msg = upstream_message(c.get_season(42, 1).await.unwrap_err()).await;
        assert_eq!(
            msg,
            "TMDB is rate-limiting requests right now. Please try again in a moment."
        );
        let msg = upstream_message(c.get_season(42, 2).await.unwrap_err()).await;
        assert_eq!(
            msg,
            "TMDB is unavailable right now. Please try again shortly."
        );
    }

    // @spec TMDB-ERR-004
    #[tokio::test]
    async fn get_season_other_status_names_the_season() {
        let server = MockServer::start().await;
        mock_status(&server, "/tv/42/season/3", 404).await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let msg = upstream_message(c.get_season(42, 3).await.unwrap_err()).await;
        assert_eq!(msg, "TMDB season 3 returned 404 Not Found");
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

    // @spec TMDB-ERR-002
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

    // @spec TMDB-ERR-003
    #[tokio::test]
    async fn get_movie_500_maps_to_unavailable_message() {
        let server = MockServer::start().await;
        mock_status(&server, "/movie/9", 500).await;
        let c = TmdbClient::with_base_url("k".into(), server.uri());
        let msg = upstream_message(c.get_movie(9).await.unwrap_err()).await;
        assert_eq!(
            msg,
            "TMDB is unavailable right now. Please try again shortly."
        );
    }

    // @spec TMDB-CLIENT-007
    #[tokio::test]
    async fn search_multi_sends_query_key_and_adult_filter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/multi"))
            .and(query_param("api_key", "test_key"))
            .and(query_param("query", "the bear"))
            .and(query_param("include_adult", "false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [
                    {"id": 1, "media_type": "tv", "name": "The Bear"},
                    {"id": 2, "media_type": "person", "name": "Someone"}
                ]
            })))
            .mount(&server)
            .await;
        let c = TmdbClient::with_base_url("test_key".into(), server.uri());
        let body = c.search_multi("the bear").await.unwrap();
        assert_eq!(body.results.len(), 2);
        assert_eq!(body.results[0].name.as_deref(), Some("The Bear"));
        assert_eq!(body.results[1].media_type.as_deref(), Some("person"));
    }

    // @spec TMDB-ERR-005
    #[tokio::test]
    async fn transport_error_does_not_leak_api_key() {
        // Unreachable host → reqwest transport error, which would normally carry
        // the full request URL (including `?api_key=...`) in its Display. The
        // key must never survive into the error string handlers surface.
        let c = TmdbClient::with_base_url("SUPER_SECRET_KEY".into(), "http://127.0.0.1:1".into());
        let err = c.get_show(42).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::Http(_)));
        let rendered = err.to_string();
        assert!(
            !rendered.contains("SUPER_SECRET_KEY"),
            "api key leaked into error: {rendered}"
        );
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
