pub mod api_docs;
pub mod auth;
pub mod db;
pub mod dto;
pub mod email;
pub mod error;

pub mod exports_db;
pub mod handlers;
pub mod images_db;
pub mod jwt_manager;
pub mod llm;
pub mod logs_db;
pub mod metrics;
pub mod middleware;
pub mod rate_limit;
pub mod security;
pub mod services;
pub mod utils;

#[path = "../tests/common/mod.rs"]
pub mod common;

use rate_limit::RateLimitState;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub postgres: PgPool,
    pub rate_limit: RateLimitState,
    pub metrics: metrics::Metrics,
    pub mongodb: mongodb::Client,
    pub webauthn: std::sync::Arc<webauthn_rs::Webauthn>,
    pub google_oauth: Option<services::GoogleOAuthClient>,
    pub oauth_state_store: std::sync::Arc<handlers::OAuthStateStore>,
    pub user_cache: moka::future::Cache<String, db::UserRecord>,
}
