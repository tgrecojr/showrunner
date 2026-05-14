use serde::{Deserialize, Serialize};
use sqlx::FromRow;

const PROFILE_BASE: &str = "https://image.tmdb.org/t/p/w185";

pub fn profile_url(path: Option<&str>) -> Option<String> {
    path.filter(|p| !p.is_empty())
        .map(|p| format!("{}{}", PROFILE_BASE, p))
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct MovieRow {
    pub tmdb_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i64>,
    pub added_at: String,
}

#[derive(Debug, Serialize)]
pub struct MovieWatchlistItem {
    pub tmdb_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i64>,
    pub added_at: String,
}

#[derive(Debug, Serialize)]
pub struct CastMember {
    pub name: String,
    pub character: Option<String>,
    pub profile_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MovieDetail {
    pub tmdb_id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i64>,
    pub watch_providers: Vec<String>,
    pub directors: Vec<String>,
    pub cast: Vec<CastMember>,
}

#[derive(Debug, Deserialize)]
pub struct AddMovieRequest {
    pub tmdb_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_movie_request_deserializes() {
        let req: AddMovieRequest = serde_json::from_str(r#"{"tmdb_id":27205}"#).unwrap();
        assert_eq!(req.tmdb_id, 27205);
    }
}
