use std::collections::HashMap;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// Application-level error type for services and handlers.
#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Forbidden(String),
    Unauthorized(String),
    Conflict(String),
    TooManyRequests(String),
    Internal(String),
    Validation {
        message: String,
        fields: HashMap<String, Vec<String>>,
    },
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "{msg}"),
            AppError::BadRequest(msg) => write!(f, "{msg}"),
            AppError::Forbidden(msg) => write!(f, "{msg}"),
            AppError::Unauthorized(msg) => write!(f, "{msg}"),
            AppError::Conflict(msg) => write!(f, "{msg}"),
            AppError::TooManyRequests(msg) => write!(f, "{msg}"),
            AppError::Internal(msg) => write!(f, "{msg}"),
            AppError::Validation { message, .. } => write!(f, "{message}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, json!({ "error": msg })),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, json!({ "error": msg })),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, json!({ "error": msg })),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, json!({ "error": msg })),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, json!({ "error": msg })),
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, json!({ "error": msg })),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": msg })),
            AppError::Validation { message, fields } => {
                (StatusCode::BAD_REQUEST, json!({ "error": message, "fields": fields }))
            }
        };
        (status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

/// Database-level error type. Wraps sqlx errors with domain-specific variants.
#[derive(Debug)]
pub enum DbError {
    NotFound(String),
    UniqueViolation(String),
    ForeignKeyViolation(String),
    Internal(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::NotFound(msg) => write!(f, "{msg}"),
            DbError::UniqueViolation(msg) => write!(f, "{msg}"),
            DbError::ForeignKeyViolation(msg) => write!(f, "{msg}"),
            DbError::Internal(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<DbError> for AppError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::NotFound(msg) => AppError::NotFound(msg),
            DbError::UniqueViolation(msg) => AppError::Conflict(msg),
            DbError::ForeignKeyViolation(msg) => AppError::BadRequest(msg),
            DbError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl From<anyhow::Error> for DbError {
    fn from(err: anyhow::Error) -> Self {
        DbError::Internal(err.to_string())
    }
}

impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => DbError::NotFound(err.to_string()),
            sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                DbError::UniqueViolation(err.to_string())
            }
            sqlx::Error::Database(ref db_err) if db_err.is_foreign_key_violation() => {
                DbError::ForeignKeyViolation(err.to_string())
            }
            _ => DbError::Internal(err.to_string()),
        }
    }
}

impl From<std::fmt::Error> for DbError {
    fn from(err: std::fmt::Error) -> Self {
        DbError::Internal(err.to_string())
    }
}
