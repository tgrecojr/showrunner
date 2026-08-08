use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP request error: {0}")]
    Http(reqwest::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Upstream service unavailable: {0}")]
    Upstream(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Too many requests: {0}")]
    TooManyRequests(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl From<reqwest::Error> for AppError {
    /// Strip the request URL before storing the error. `reqwest::Error`'s
    /// `Display` appends " for url (...)", and our outbound URLs carry secrets
    /// — the TMDB API key as a query param. Some handlers surface error
    /// strings to clients and logs, so the URL must never reach the error
    /// object in the first place.
    fn from(err: reqwest::Error) -> Self {
        AppError::Http(err.without_url())
    }
}

impl AppError {
    /// A message safe to hand back to clients. User-facing variants keep their
    /// text; internal variants (DB / HTTP / IO / JSON) collapse to a generic
    /// string so sqlx and reqwest internals — including anything the URL-strip
    /// in `From<reqwest::Error>` didn't cover — never reach a response body.
    /// Use this anywhere an error is serialized into a `200 OK` body (e.g. the
    /// per-item results of `/sync`), not just the `IntoResponse` error path.
    pub fn client_message(&self) -> String {
        match self {
            AppError::NotFound(msg)
            | AppError::InvalidData(msg)
            | AppError::Config(msg)
            | AppError::TooManyRequests(msg)
            | AppError::Upstream(msg) => msg.clone(),
            AppError::Database(_) | AppError::Http(_) | AppError::Io(_) | AppError::Json(_) => {
                "An internal error occurred".to_string()
            }
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::InvalidData(_) => StatusCode::BAD_REQUEST,
            AppError::Upstream(_) => StatusCode::BAD_GATEWAY,
            AppError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if matches!(
            self,
            AppError::Database(_) | AppError::Http(_) | AppError::Io(_) | AppError::Json(_)
        ) {
            tracing::error!("Internal error: {}", self);
        }

        let body = serde_json::json!({ "error": self.client_message() });
        (self.status_code(), axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn not_found_maps_to_404_with_message() {
        let resp = AppError::NotFound("show 7 missing".into()).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            body_json(resp).await,
            serde_json::json!({"error": "show 7 missing"})
        );
    }

    #[tokio::test]
    async fn invalid_data_maps_to_400() {
        let resp = AppError::InvalidData("bad".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_json(resp).await, serde_json::json!({"error": "bad"}));
    }

    #[tokio::test]
    async fn config_maps_to_500_with_message() {
        let resp = AppError::Config("missing".into()).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body_json(resp).await,
            serde_json::json!({"error": "missing"})
        );
    }

    #[tokio::test]
    async fn upstream_maps_to_502() {
        let resp = AppError::Upstream("tmdb 500".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            body_json(resp).await,
            serde_json::json!({"error": "tmdb 500"})
        );
    }

    #[tokio::test]
    async fn database_error_hidden_from_response_body() {
        let inner = sqlx::Error::RowNotFound;
        let resp = AppError::Database(inner).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        // Body should be the generic message, not the inner sqlx detail.
        assert_eq!(
            body_json(resp).await,
            serde_json::json!({"error": "An internal error occurred"})
        );
    }

    #[test]
    fn from_io_error() {
        let io = std::io::Error::other("disk gone");
        let err: AppError = io.into();
        assert!(matches!(err, AppError::Io(_)));
        assert!(err.to_string().contains("disk gone"));
    }

    #[test]
    fn from_json_error() {
        let parse_err = serde_json::from_str::<i64>("not json").unwrap_err();
        let err: AppError = parse_err.into();
        assert!(matches!(err, AppError::Json(_)));
    }
}
