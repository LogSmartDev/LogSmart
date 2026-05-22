use crate::{
    AppState,
    db::{self, UserRole},
    dto::{
        ClockEventResponse, ClockStatusResponse, CompanyClockEventResponse,
        CompanyClockEventsResponse, ErrorResponse,
    },
    middleware::{AnyAuthUser, ReadCompanyUser},
    services,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

#[utoipa::path(
    post,
    path = "/clock/in",
    responses(
        (status = 200, description = "Successfully clocked in", body = ClockEventResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 409, description = "Already clocked in", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Clock In/Out"
)]
/// Clocks in the current user.
///
/// # Errors
/// Returns an error if the user is already clocked in or if DB operations fail.
pub async fn clock_in(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
) -> Result<Json<ClockEventResponse>, crate::error::AppError> {
    let company_id = db::get_user_company_id(&state.postgres, &user.id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let event = services::ClockService::clock_in(&state.postgres, &user.id, &company_id)
        .await
        ?;

    Ok(Json(ClockEventResponse::from(event)))
}

#[utoipa::path(
    post,
    path = "/clock/out",
    responses(
        (status = 200, description = "Successfully clocked out", body = ClockEventResponse),
        (status = 400, description = "Not currently clocked in", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Clock In/Out"
)]
/// Clocks out the current user.
///
/// # Errors
/// Returns an error if the user is not clocked in or if DB operations fail.
pub async fn clock_out(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
) -> Result<Json<ClockEventResponse>, crate::error::AppError> {
    let event = services::ClockService::clock_out(&state.postgres, &user.id)
        .await
        ?;

    Ok(Json(ClockEventResponse::from(event)))
}

#[utoipa::path(
    get,
    path = "/clock/status",
    responses(
        (status = 200, description = "Current clock status and recent events", body = ClockStatusResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Clock In/Out"
)]
/// Gets the current clock status and last 5 events for the user.
///
/// # Errors
/// Returns an error if DB operations fail.
pub async fn get_clock_status(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
) -> Result<Json<ClockStatusResponse>, crate::error::AppError> {
    let (current, recent) = services::ClockService::get_status(&state.postgres, &user.id)
        .await
        ?;

    let is_clocked_in = current
        .as_ref()
        .is_some_and(super::super::db::ClockEvent::is_clocked_in);
    let current_event = if is_clocked_in {
        current.map(ClockEventResponse::from)
    } else {
        None
    };

    let recent_events = recent.into_iter().map(ClockEventResponse::from).collect();

    Ok(Json(ClockStatusResponse {
        is_clocked_in,
        current_event,
        recent_events,
    }))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct CompanyClockQuery {
    /// ISO 8601 start date filter (inclusive)
    pub from: Option<String>,
    /// ISO 8601 end date filter (inclusive)
    pub to: Option<String>,
    /// Branch ID filter (optional, for `company_manager` to filter by specific branch)
    pub branch_id: Option<String>,
    /// Number of results per page (default 25, max 100)
    pub limit: Option<i64>,
    /// Cursor for pagination (base64 encoded timestamp|id)
    pub cursor: Option<String>,
}

#[utoipa::path(
    get,
    path = "/clock/company",
    params(CompanyClockQuery),
    responses(
        (status = 200, description = "All company clock events", body = CompanyClockEventsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden – admin only", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Clock In/Out"
)]
/// Gets all clock in/out events for the admin's company.
///
/// # Errors
/// Returns an error if the user is not an admin or if DB operations fail.
pub async fn get_company_clock_events(
    ReadCompanyUser(_claims, user): ReadCompanyUser,
    State(state): State<AppState>,
    Query(params): Query<CompanyClockQuery>,
) -> Result<Json<CompanyClockEventsResponse>, crate::error::AppError> {
    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let from = params
        .from
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let to = params
        .to
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    // If user is branch_manager, restrict to their branch only
    let branch_id_filter = if user.role == UserRole::BranchManager {
        user.branch_id
    } else {
        params.branch_id
    };

    let (events, next_cursor) = services::ClockService::get_company_clock_events(
        &state.postgres,
        &company_id,
        from,
        to,
        branch_id_filter,
        params.limit,
        params.cursor,
    )
    .await
    ?;

    let events = events
        .into_iter()
        .map(CompanyClockEventResponse::from)
        .collect();
    Ok(Json(CompanyClockEventsResponse {
        events,
        next_cursor,
    }))
}
