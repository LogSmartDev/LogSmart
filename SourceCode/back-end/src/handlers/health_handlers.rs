use crate::middleware::LogSmartAdminUser;
use crate::rate_limit::{
    get_general_ip_limit, get_login_email_limit, get_login_ip_limit, get_register_email_limit,
    get_register_ip_limit,
};
use crate::{AppState, db, utils};
use axum::extract::State;
use axum::response::{IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize, utoipa::ToSchema)]
pub struct BasicHealthResponse {
    pub status: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub metrics: db::DatabaseHealthMetrics,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SlowQueriesResponse {
    pub queries: Vec<db::SlowQueryInfo>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct IndexUsageResponse {
    pub indexes: Vec<db::IndexUsageStats>,
    pub unused_indexes: Vec<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TableSizesResponse {
    pub tables: Vec<db::TableSizeInfo>,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = BasicHealthResponse),
    ),
    tag = "Health Monitoring"
)]
pub async fn basic_health_check() -> impl IntoResponse {
    Json(BasicHealthResponse {
        status: "ok".to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/health/database",
    responses(
        (status = 200, description = "Database health metrics retrieved successfully", body = HealthResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - LogSmart admin only"),
        (status = 500, description = "Server error"),
    ),
    security(("bearer_auth" = [])),
    tag = "Health Monitoring"
)]
pub async fn get_db_health(
    LogSmartAdminUser(_claims, _user): LogSmartAdminUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match db::get_database_health(&state.postgres).await {
        Ok(metrics) => Json(HealthResponse {
            status: "healthy".to_string(),
            metrics,
        })
        .into_response(),
        Err(e) => utils::err_internal(&format!("Failed to get health metrics: {e}")).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/health/slow-queries",
    params(
        ("limit" = Option<i64>, Query, description = "Maximum number of slow queries to return (default: 20)")
    ),
    responses(
        (status = 200, description = "Slow queries retrieved successfully", body = SlowQueriesResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - LogSmart admin only"),
        (status = 500, description = "Server error"),
    ),
    security(("bearer_auth" = [])),
    tag = "Health Monitoring"
)]
#[allow(clippy::implicit_hasher)]
pub async fn get_db_slow_queries(
    LogSmartAdminUser(_claims, _user): LogSmartAdminUser,
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(20);

    match db::get_slow_queries(&state.postgres, limit).await {
        Ok(queries) => Json(SlowQueriesResponse { queries }).into_response(),
        Err(e) => utils::err_internal(&format!("Failed to get slow queries: {e}")).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/health/index-usage",
    responses(
        (status = 200, description = "Index usage statistics retrieved successfully", body = IndexUsageResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - LogSmart admin only"),
        (status = 500, description = "Server error"),
    ),
    security(("bearer_auth" = [])),
    tag = "Health Monitoring"
)]
pub async fn get_db_index_usage(
    LogSmartAdminUser(_claims, _user): LogSmartAdminUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let indexes = match db::get_index_usage(&state.postgres).await {
        Ok(idx) => idx,
        Err(e) => {
            return utils::err_internal(&format!("Failed to get index usage: {e}")).into_response();
        }
    };

    let unused = match db::check_unused_indexes(&state.postgres).await {
        Ok(un) => un,
        Err(e) => {
            return utils::err_internal(&format!("Failed to check unused indexes: {e}")).into_response();
        }
    };

    Json(IndexUsageResponse {
        indexes,
        unused_indexes: unused,
    })
    .into_response()
}

#[utoipa::path(
    get,
    path = "/health/table-sizes",
    responses(
        (status = 200, description = "Table sizes retrieved successfully", body = TableSizesResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - LogSmart admin only"),
        (status = 500, description = "Server error"),
    ),
    security(("bearer_auth" = [])),
    tag = "Health Monitoring"
)]
pub async fn get_db_table_sizes(
    LogSmartAdminUser(_claims, _user): LogSmartAdminUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match db::get_table_sizes(&state.postgres).await {
        Ok(tables) => Json(TableSizesResponse { tables }).into_response(),
        Err(e) => utils::err_internal(&format!("Failed to get table sizes: {e}")).into_response(),
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct RateLimitStatusResponse {
    pub enabled: bool,
    pub login_ip_limit: u32,
    pub register_ip_limit: u32,
    pub general_ip_limit: u32,
    pub login_email_limit: u32,
    pub register_email_limit: u32,
    pub export_limit: u32,
}

#[utoipa::path(
    get,
    path = "/health/rate-limits",
    responses(
        (status = 200, description = "Rate limiting status retrieved successfully", body = RateLimitStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - LogSmart admin only"),
    ),
    security(("bearer_auth" = [])),
    tag = "Health Monitoring"
)]
pub async fn get_rate_limit_status(
    LogSmartAdminUser(_claims, _user): LogSmartAdminUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rate_limit = &state.rate_limit;
    Json(RateLimitStatusResponse {
        enabled: !rate_limit.disabled,
        login_ip_limit: get_login_ip_limit(),
        register_ip_limit: get_register_ip_limit(),
        general_ip_limit: get_general_ip_limit(),
        login_email_limit: get_login_email_limit(),
        register_email_limit: get_register_email_limit(),
        export_limit: 1,
    })
    .into_response()
}
