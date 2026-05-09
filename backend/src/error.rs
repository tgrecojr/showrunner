use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

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
}

pub type Result<T> = std::result::Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::InvalidData(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Config(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Upstream(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            other => {
                tracing::error!("Internal error: {}", other);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred".to_string(),
                )
            }
        };

        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
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
