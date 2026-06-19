use crate::{
    AppState,
    auth::generate_uuid6_token,
    db,
    dto::{
        BranchDto, ConfirmBranchDeletionRequest, ConfirmBranchDeletionResponse,
        CreateBranchRequest, ErrorResponse, ListBranchesResponse, RequestBranchDeletionRequest,
        RequestBranchDeletionResponse, UpdateBranchRequest,
    },
    email,
    middleware::{AuditRequestContext, ManageCompanyUser, ReadCompanyUser},
    utils::{err_validation, friendly_validation_errors, AuditLogger},
};
use axum::{Json, extract::State, http::StatusCode};
use validator::Validate;

#[utoipa::path(
    post,
    path = "/auth/company/branches",
    request_body = CreateBranchRequest,
    responses(
        (status = 201, description = "Branch created successfully", body = BranchDto),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - only company manager can create branches", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company Management"
)]
/// Creates a new branch for the user's company.
pub async fn create_branch(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    Json(payload): Json<CreateBranchRequest>,
) -> Result<(StatusCode, Json<BranchDto>), crate::error::AppError> {
    // Validate request payload
    payload
        .validate()
        .map_err(|e| {
            let (msg, fields) = friendly_validation_errors(&e);
            err_validation(&msg, fields)
        })?;

    let company_id = user
        .company_id
        .clone()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    let branch = db::create_branch(
        &state.postgres,
        company_id.clone(),
        payload.name,
        payload.address,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error: {:?}", e);
        crate::error::AppError::Internal("Failed to create branch".to_string())
    })?;

    AuditLogger::log(
        &state.postgres,
        "branch_created",
        Some(user.id.clone()),
        Some(user.email.clone()),
        crate::audit_ctx!(
            &audit_ctx,
            actor: &user,
            company_id: Some(company_id),
            target_user_id: Some(branch.id.clone())
        ),
        Some(format!("Created branch: {}", branch.name)),
        true,
    )
    .await;

    Ok((StatusCode::CREATED, Json(branch.into())))
}

#[utoipa::path(
    get,
    path = "/auth/company/branches",
    responses(
        (status = 200, description = "Branches retrieved successfully", body = ListBranchesResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company Management"
)]
/// Lists all branches for the user's company.
pub async fn list_branches(
    ReadCompanyUser(_claims, user): ReadCompanyUser,
    State(state): State<AppState>,
) -> Result<Json<ListBranchesResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .clone()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;
    let branches =
        db::get_branches_by_company_id_with_deletion_status(&state.postgres, &company_id)
            .await
            .map_err(|e| {
                tracing::error!("Error: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?;

    Ok(Json(ListBranchesResponse {
        branches: branches.into_iter().map(BranchDto::from).collect(),
    }))
}

#[utoipa::path(
    put,
    path = "/auth/company/branches",
    request_body = UpdateBranchRequest,
    responses(
        (status = 200, description = "Branch updated successfully", body = BranchDto),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - only company manager can update branches", body = ErrorResponse),
        (status = 404, description = "Branch not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company Management"
)]
/// Updates an existing branch for the user's company.
pub async fn update_branch(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    Json(payload): Json<UpdateBranchRequest>,
) -> Result<Json<BranchDto>, crate::error::AppError> {
    // Validate request payload
    payload
        .validate()
        .map_err(|e| {
            let (msg, fields) = friendly_validation_errors(&e);
            err_validation(&msg, fields)
        })?;

    let company_id = user
        .company_id
        .clone()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    // Verify the branch belongs to the user's company
    let branch = db::get_branch_by_id(&state.postgres, &payload.branch_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .ok_or(crate::error::AppError::NotFound(
            "Branch not found".to_string(),
        ))?;

    if branch.company_id != company_id {
        return Err(crate::error::AppError::Forbidden(
            "Branch does not belong to your company".to_string(),
        ));
    }

    let updated_branch = db::update_branch(
        &state.postgres,
        &payload.branch_id,
        &payload.name,
        &payload.address,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error: {:?}", e);
        crate::error::AppError::Internal("Failed to update branch".to_string())
    })?;

    AuditLogger::log(
        &state.postgres,
        "branch_updated",
        Some(user.id.clone()),
        Some(user.email.clone()),
        crate::audit_ctx!(
            &audit_ctx,
            actor: &user,
            company_id: Some(company_id),
            target_user_id: Some(updated_branch.id.clone())
        ),
        Some(format!("Updated branch: {}", updated_branch.name)),
        true,
    )
    .await;

    Ok(Json(updated_branch.into()))
}

#[utoipa::path(
    post,
    path = "/auth/company/branches/request-deletion",
    request_body = RequestBranchDeletionRequest,
    responses(
        (status = 200, description = "Branch deletion confirmation email sent", body = RequestBranchDeletionResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - only company manager can request branch deletion", body = ErrorResponse),
        (status = 404, description = "Branch not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company Management"
)]
/// Requests branch deletion by sending a confirmation email.
pub async fn request_branch_deletion(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    Json(payload): Json<RequestBranchDeletionRequest>,
) -> Result<Json<RequestBranchDeletionResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .as_ref()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    let branch = db::get_branch_by_id(&state.postgres, &payload.branch_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .ok_or(crate::error::AppError::NotFound(
            "Branch not found".to_string(),
        ))?;

    if &branch.company_id != company_id {
        return Err(crate::error::AppError::Forbidden(
            "Branch does not belong to your company".to_string(),
        ));
    }

    let token = generate_uuid6_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);

    db::create_branch_deletion_token(
        &state.postgres,
        user.id.clone(),
        payload.branch_id,
        token.clone(),
        expires_at,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error: {:?}", e);
        crate::error::AppError::Internal("Failed to process deletion request".to_string())
    })?;

    let confirmation_link = format!(
        "{}/confirm-branch-deletion?token={}",
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://logsmart.app".to_string()),
        token
    );

    email::send_branch_deletion_confirmation_email(&user.email, &branch.name, &confirmation_link)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to send confirmation email".to_string())
        })?;

    AuditLogger::log(
        &state.postgres,
        "branch_deletion_requested",
        Some(user.id.clone()),
        Some(user.email.clone()),
        crate::audit_ctx!(
            &audit_ctx,
            actor: &user,
            company_id: user.company_id.clone(),
            target_user_id: Some(branch.id)
        ),
        Some(format!("Requested deletion for branch: {}", branch.name)),
        true,
    )
    .await;

    Ok(Json(RequestBranchDeletionResponse {
        message: "A confirmation email has been sent to your email address. Please check your inbox to complete the branch deletion.".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/auth/company/branches/confirm-deletion",
    request_body = ConfirmBranchDeletionRequest,
    responses(
        (status = 200, description = "Branch deleted successfully", body = ConfirmBranchDeletionResponse),
        (status = 401, description = "Invalid or expired token", body = ErrorResponse),
        (status = 404, description = "Branch not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    tag = "Company Management"
)]
/// Confirms and executes branch deletion using a token.
pub async fn confirm_branch_deletion(
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    Json(payload): Json<ConfirmBranchDeletionRequest>,
) -> Result<Json<ConfirmBranchDeletionResponse>, crate::error::AppError> {
    let token_record = db::get_branch_deletion_token(&state.postgres, &payload.token)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?;

    let (token_id, user_id, branch_id) = token_record.ok_or(
        crate::error::AppError::Unauthorized("Invalid or expired confirmation token".to_string()),
    )?;

    let branch = db::get_branch_by_id(&state.postgres, &branch_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?;

    let branch_name = match &branch {
        Some(b) => b.name.clone(),
        None => "Unknown".to_string(),
    };

    db::delete_branch(&state.postgres, &branch_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to delete branch".to_string())
        })?;

    db::mark_branch_deletion_token_used(&state.postgres, &token_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to complete deletion".to_string())
        })?;

    let user = db::get_user_by_id(&state.postgres, &user_id)
        .await
        .ok()
        .flatten();

    if let Some(user) = user {
        AuditLogger::log(
            &state.postgres,
            "branch_deletion_confirmed",
            Some(user.id.clone()),
            Some(user.email.clone()),
            crate::audit_ctx!(
                &audit_ctx,
                actor: &user,
                company_id: user.company_id.clone(),
                target_user_id: Some(branch_id.clone())
            ),
            Some(format!("Confirmed deletion for branch: {}", branch_name)),
            true,
        )
        .await;

        let _ = email::send_branch_deleted_notification_email(&user.email, &branch_name).await;
    }

    Ok(Json(ConfirmBranchDeletionResponse {
        message: format!("Branch '{branch_name}' has been successfully deleted."),
    }))
}
