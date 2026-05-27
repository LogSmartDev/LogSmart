use crate::dto::GetPendingInvitationsResponse;
use crate::middleware::{
    AuditRequestContext, BranchManagerUser, ManageCompanyUser, ReadBranchUser,
};
use crate::utils::{extract_ip_from_headers_and_addr, extract_user_agent};
use crate::{
    AppState,
    auth::{hash_password, validate_password_policy},
    db,
    dto::{
        AcceptInvitationRequest, AuthResponse, CancelInvitationRequest, ErrorResponse,
        GetInvitationDetailsRequest, GetInvitationDetailsResponse, InvitationResponse,
        InviteUserRequest,
    },
    jwt_manager::JwtManager,
    services,
    utils::AuditLogger,
};
use axum::{
    Json,
    extract::{ConnectInfo, Query, State},
    http::{self, HeaderMap, StatusCode, header::SET_COOKIE},
    response::IntoResponse,
};
use serde_json::json;
use validator::Validate;

#[utoipa::path(
    post,
    path = "/auth/invitations/send",
    request_body = InviteUserRequest,
    responses(
        (status = 201, description = "Invitation sent successfully", body = InvitationResponse),
        (status = 400, description = "Invalid request data", body = ErrorResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 409, description = "User already invited or exists", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Invitations"
)]
/// Sends an invitation to a new user.
///
/// # Errors
/// Returns an error if the user is not authorized, the email is invalid, or the invitation fails to send.
pub async fn invite_user(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<InviteUserRequest>,
) -> Result<(StatusCode, Json<InvitationResponse>), crate::error::AppError> {
    let _timer = crate::metrics::RequestTimer::new();
    state.metrics.increment_total_requests();

    let ip_address = extract_ip_from_headers_and_addr(&headers, &addr);
    let user_agent = extract_user_agent(&headers);

    // Validate request payload
    payload
        .validate()
        .map_err(|e| crate::error::AppError::BadRequest(format!("Validation failed: {e}")))?;

    // Branch managers can only invite staff to their own branch
    if user.is_branch_manager() {
        if payload.branch_id.is_none() {
            return Err(crate::error::AppError::Forbidden(
                "Branch managers cannot invite company-wide users".to_string(),
            ));
        }
        if payload.branch_id != user.branch_id {
            return Err(crate::error::AppError::Forbidden(
                "Branch managers can only invite users to their own branch".to_string(),
            ));
        }
        if let Some(role) = &payload.role
            && *role != db::UserRole::Staff
        {
            return Err(crate::error::AppError::Forbidden(
                "Branch managers can only invite staff members".to_string(),
            ));
        }
    }

    let (invitation_id, expires_at) = services::InvitationService::send_invitation(
        &state.postgres,
        &user,
        payload.email.clone(),
        payload.role.unwrap_or(db::UserRole::Staff),
        payload.branch_id,
        Some(ip_address),
        user_agent,
    )
    .await
    .inspect_err(|_e| {
        state.metrics.increment_failed_requests();
    })?;

    state.metrics.increment_invitations_sent();
    state.metrics.increment_successful_requests();

    Ok((
        StatusCode::CREATED,
        Json(InvitationResponse {
            id: invitation_id,
            email: payload.email,
            expires_at,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/auth/invitations/accept",
    request_body = AcceptInvitationRequest,
    responses(
        (status = 201, description = "Invitation accepted successfully", body = AuthResponse),
        (status = 400, description = "Invalid request data", body = ErrorResponse),
        (status = 401, description = "Invalid or expired invitation token", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse),
    ),
    tag = "Invitations"
)]
/// Accepts an invitation and creates a new user account.
///
/// # Errors
/// Returns an error if the token is invalid/expired, validation fails, or account creation fails.
pub async fn accept_invitation(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<AcceptInvitationRequest>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    let _timer = crate::metrics::RequestTimer::new();
    state.metrics.increment_total_requests();

    let ip_address = extract_ip_from_headers_and_addr(&headers, &addr);
    let user_agent = extract_user_agent(&headers);

    if payload.token.is_empty()
        || payload.first_name.is_empty()
        || payload.last_name.is_empty()
        || payload.password.is_empty()
    {
        return Err(crate::error::AppError::BadRequest(
            "Missing required fields".to_string(),
        ));
    }

    if let Err(e) = validate_password_policy(&payload.password) {
        return Err(crate::error::AppError::BadRequest(
            e.to_string().to_string(),
        ));
    }

    let invitation = db::get_invitation_by_token(&state.postgres, &payload.token)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .ok_or(crate::error::AppError::Unauthorized(
            "Invalid or expired invitation".to_string(),
        ))?;

    let now = chrono::Utc::now();

    if now > invitation.expires_at {
        return Err(crate::error::AppError::Unauthorized(
            "Invitation has expired".to_string(),
        ));
    }

    if db::get_user_by_email(&state.postgres, &invitation.email)
        .await
        .map_err(|e| {
            tracing::error!(
                "Database error checking existing user during invitation acceptance: {:?}",
                e
            );
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .is_some()
    {
        return Err(crate::error::AppError::Conflict(
            "Email already has an account".to_string(),
        ));
    }

    let password_hash = hash_password(&payload.password).map_err(|e| {
        tracing::error!(
            "Failed to hash password during invitation acceptance: {:?}",
            e
        );
        crate::error::AppError::Internal("Failed to process password".to_string())
    })?;

    let created_user = db::accept_invitation_with_user_creation(
        &state.postgres,
        &invitation.id,
        &invitation.email,
        payload.first_name.clone(),
        payload.last_name.clone(),
        password_hash,
        &invitation.company_id,
        invitation.role,
        invitation.branch_id,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error: {:?}", e);
        crate::error::AppError::Internal("Failed to create user".to_string())
    })?;

    let user_id = created_user.id.clone();
    let accept_audit_ctx = crate::utils::AuditContext {
        ip_address: Some(ip_address),
        user_agent,
        request_path: Some("/auth/invitations/accept".to_string()),
        request_method: Some("POST".to_string()),
        ..crate::utils::AuditContext::default()
    };

    let jwt_config = JwtManager::get_config();
    let token = jwt_config
        .generate_token(user_id.as_str(), 24)
        .map_err(|e| {
            tracing::error!(
                "Failed to generate JWT token for invitation acceptance: {:?}",
                e
            );
            crate::error::AppError::Internal("Failed to generate token".to_string())
        })?;

    AuditLogger::log_invitation_accepted(
        &state.postgres,
        user_id.clone(),
        invitation.email.clone(),
        invitation.company_id.clone(),
        crate::audit_ctx!(
            &accept_audit_ctx,
            actor: &created_user,
            company_id: Some(invitation.company_id.clone())
        ),
    )
    .await;

    state.metrics.increment_invitations_accepted();
    state.metrics.increment_successful_requests();
    tracing::info!("Invitation accepted by user: {}", user_id);

    let cookie_domain = std::env::var("COOKIE_DOMAIN").unwrap_or_default();
    let domain_attr = if cookie_domain.is_empty() {
        String::new()
    } else {
        format!("; Domain={cookie_domain}")
    };

    let cookie = format!(
        "ls-token={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}{}",
        token,
        60 * 60 * 24 * 7,
        domain_attr
    );

    let mut response = (
        StatusCode::CREATED,
        Json(AuthResponse {
            token: token.clone(),
            user: created_user.into(),
        }),
    )
        .into_response();

    response.headers_mut().insert(
        SET_COOKIE,
        http::header::HeaderValue::from_str(&cookie).map_err(|e| {
            tracing::error!(
                "Failed to set cookie in invitation acceptance response: {:?}",
                e
            );
            crate::error::AppError::Internal("Failed to set authentication cookie".to_string())
        })?,
    );

    Ok(response)
}

#[utoipa::path(
    get,
    path = "/auth/invitations/details",
    params(
        GetInvitationDetailsRequest
    ),
    responses(
        (status = 200, description = "Invitation details retrieved successfully", body = GetInvitationDetailsResponse),
        (status = 404, description = "Invitation not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    tag = "Invitations"
)]
/// Retrieves details of an invitation using its token.
///
/// # Errors
/// Returns an error if the invitation is not found or if there's a database error.
pub async fn get_invitation_details(
    State(state): State<AppState>,
    Query(payload): Query<GetInvitationDetailsRequest>,
) -> Result<Json<GetInvitationDetailsResponse>, crate::error::AppError> {
    let invitation = db::get_invitation_by_token(&state.postgres, &payload.token)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .ok_or(crate::error::AppError::NotFound(
            "Invitation not found".to_string(),
        ))?;

    let company_name = db::get_company_by_id(&state.postgres, &invitation.company_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .ok_or(crate::error::AppError::NotFound(
            "Company not found".to_string(),
        ))?
        .name;

    Ok(Json(GetInvitationDetailsResponse {
        company_name,
        expires_at: invitation.expires_at,
    }))
}

#[utoipa::path(
    get,
    path = "/auth/invitations/pending",
    responses(
        (status = 200, description = "Invitations retrieved successfully", body = [InvitationResponse]),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Invitations"
)]
/// Retrieves all pending invitations for the current user's company.
///
/// # Errors
/// Returns an error if the user is not authorized or if the query fails.
pub async fn get_pending_invitations(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
) -> Result<Json<GetPendingInvitationsResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .as_ref()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    let invitations =
        services::InvitationService::get_pending_invitations(&state.postgres, company_id)
            .await
            .map(|inv_list| {
                inv_list
                    .into_iter()
                    .filter(|inv| {
                        if user.can_manage_company() || user.is_readonly_hq() {
                            true
                        } else {
                            inv.branch_id == user.branch_id
                        }
                    })
                    .map(|inv| InvitationResponse {
                        id: inv.id,
                        email: inv.email,
                        expires_at: inv.expires_at,
                    })
                    .collect::<Vec<_>>()
            })?;

    Ok(Json(GetPendingInvitationsResponse { invitations }))
}
#[utoipa::path(
    put,
    path = "/auth/invitations/cancel",
    request_body = CancelInvitationRequest,
    responses(
        (status = 200, description = "Invitation cancelled successfully"),
        (status = 400, description = "Invalid request or invitation already accepted/cancelled", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - not an admin or different company", body = ErrorResponse),
        (status = 404, description = "Invitation not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Invitations"
)]
/// Cancels a pending invitation.
///
/// # Errors
/// Returns an error if the user is not authorized or if the cancellation fails.
pub async fn cancel_invitation(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    State(state): State<AppState>,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    Json(payload): Json<crate::dto::CancelInvitationRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    services::InvitationService::cancel_invitation(
        &state.postgres,
        &user,
        &payload.invitation_id,
        crate::audit_ctx!(&audit_ctx),
    )
    .await?;

    Ok(Json(
        json!({ "message": "Invitation cancelled successfully" }),
    ))
}
