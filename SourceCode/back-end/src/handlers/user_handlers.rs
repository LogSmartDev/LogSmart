use crate::{
    AppState, db,
    dto::{
        AdminProfilePictureQuery, AdminUpdateMemberRequest, ErrorResponse,
        GetCompanyMembersResponse, RemoveMemberRequest,
    },
    images_db,
    middleware::{AnyAuthUser, AuditRequestContext, BranchManagerUser, ReadBranchUser},
    services::user_service::UserService,
    utils::{
        AuditLogger, err_bad_request, err_forbidden, err_internal, err_not_found,
        infer_content_type,
    },
};
use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::{StatusCode, header},
};
use serde_json::json;

#[utoipa::path(
    get,
    path = "/auth/company/members",
    responses(
        (status = 200, description = "Company members retrieved successfully", body = [GetCompanyMembersResponse]),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company Management"
)]
/// Retrieves all members belonging to the current user's company.
///
/// # Errors
/// Returns an error if the user is not associated with a company or if the database query fails.
pub async fn get_company_members(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
) -> Result<Json<GetCompanyMembersResponse>, crate::error::AppError> {
    tracing::info!("get_company_members called for user_id: {}", user.id);

    let members = db::get_company_members_for_user(&state.postgres, &user.id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company members: {:?}", e);
            err_internal("Database error")
        })?;

    if members.is_empty() {
        return Err(err_forbidden("User is not associated with a company"));
    }

    let filtered_members = if user.can_manage_company() || user.is_readonly_hq() {
        members
    } else if user.is_branch_manager() {
        members
            .into_iter()
            .filter(|m| m.branch_id == user.branch_id)
            .collect::<Vec<_>>()
    } else {
        members.into_iter().filter(|m| m.id == user.id).collect()
    };

    Ok(Json(filtered_members.into()))
}

#[utoipa::path(
    put,
    path = "/auth/admin/update-member",
    request_body = AdminUpdateMemberRequest,
    responses(
        (status = 200, description = "Member profile updated successfully", body = GetCompanyMembersResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - not an admin or different company", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company Management"
)]
/// Updates a company member's profile (admin only).
///
/// # Errors
/// Returns an error if the user is not an admin, the request is invalid, or the update fails.
pub async fn admin_update_member_profile(
    BranchManagerUser(_claims, user): BranchManagerUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    Json(payload): Json<AdminUpdateMemberRequest>,
) -> Result<Json<GetCompanyMembersResponse>, crate::error::AppError> {
    if payload.first_name.is_empty() || payload.last_name.is_empty() {
        return Err(err_bad_request("First name and last name cannot be empty"));
    }

    let role = match payload.role.as_str() {
        "logsmart_admin" => db::UserRole::LogSmartAdmin,
        "company_manager" => db::UserRole::CompanyManager,
        "branch_manager" => db::UserRole::BranchManager,
        "staff" => db::UserRole::Staff,
        _ => {
            return Err(err_bad_request(
                "Invalid role. Must be 'logsmart_admin', 'company_manager', 'branch_manager', or 'staff'",
            ));
        }
    };

    let updated_user = UserService::admin_update_member_profile(
        &state.postgres,
        &user,
        &payload.email,
        payload.first_name.clone(),
        payload.last_name.clone(),
        role,
        payload.branch_id,
        payload.profile_picture_id,
    )
    .await?;

    // Invalidate cache for the updated user
    state.user_cache.invalidate(&updated_user.id).await;

    AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Updated member profile: {}", updated_user.email),
        crate::audit_ctx!(
            &audit_ctx,
            actor: &user,
            target_user_id: Some(updated_user.id.clone()),
            target_email: Some(updated_user.email.clone())
        ),
    )
    .await;

    Ok(Json(vec![updated_user].into()))
}
#[utoipa::path(
    delete,
    path = "/auth/admin/remove-member",
    request_body = RemoveMemberRequest,
    responses(
        (status = 200, description = "Member deleted successfully"),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - not an admin or different company", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company Management"
)]
/// Deletes a company member (admin only).
///
/// # Errors
/// Returns an error if the user is not an admin or if the deletion fails.
pub async fn admin_delete_member(
    BranchManagerUser(_claims, user): BranchManagerUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    Json(payload): Json<RemoveMemberRequest>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let deleted_user_id =
        UserService::admin_delete_member(&state.postgres, &user, &payload.email).await?;

    // Invalidate cache for the deleted user
    state.user_cache.invalidate(&deleted_user_id).await;

    AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Deleted member: {}", payload.email),
        crate::audit_ctx!(
            &audit_ctx,
            actor: &user,
            target_user_id: Some(deleted_user_id),
            target_email: Some(payload.email)
        ),
    )
    .await;

    Ok(Json(json!({ "message": "Member deleted successfully" })))
}

#[utoipa::path(
    post,
    path = "/auth/profile-picture",
    params(AdminProfilePictureQuery),
    request_body = Vec<u8>,
    responses(
        (status = 200, description = "Profile picture uploaded successfully", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "User"
)]
pub async fn upload_profile_picture(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    Query(query): Query<AdminProfilePictureQuery>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let data = body.to_vec();

    if data.len() > 10 * 1024 * 1024 {
        return Err(err_bad_request("File too large. Maximum size is 10MB"));
    }

    if data.is_empty() {
        return Err(err_bad_request("No file provided"));
    }

    let content_type = infer_content_type(&data);
    if !content_type.starts_with("image/") {
        return Err(err_bad_request("File must be an image"));
    }

    let (target_user_id, target_picture_id) = if query.email.as_deref().unwrap_or("").is_empty() {
        (user.id.clone(), user.profile_picture_id.clone())
    } else {
        if !user.can_manage_branch() {
            return Err(err_forbidden("Only managers can update member profiles"));
        }

        let target =
            UserService::get_user_by_email(&state.postgres, query.email.as_deref().unwrap_or(""))
                .await?;

        if user.is_company_manager() && user.company_id != target.company_id {
            return Err(err_forbidden("Cannot update users from other companies"));
        }

        if user.is_branch_manager() && user.branch_id != target.branch_id {
            return Err(err_forbidden(
                "Branch managers can only manage users in their branch",
            ));
        }

        if target.is_logsmart_admin() && !user.is_logsmart_admin() {
            return Err(err_forbidden("Cannot modify LogSmart internal admin users"));
        }

        (target.id, target.profile_picture_id)
    };

    let file_id =
        images_db::upload_profile_picture(&state.mongodb, data, &target_user_id, &content_type)
            .await
            .map_err(|e| {
                tracing::error!("Failed to upload profile picture: {:?}", e);
                err_internal("Failed to upload profile picture")
            })?;

    if let Err(err) =
        db::update_user_profile_picture_id(&state.postgres, &target_user_id, Some(&file_id)).await
    {
        tracing::error!("Failed to update user profile picture: {:?}", err);
        if let Err(delete_err) = images_db::delete_profile_picture(&state.mongodb, &file_id).await {
            tracing::error!(
                "Failed to cleanup uploaded profile picture: {:?}",
                delete_err
            );
        }
        return Err(err_internal("Failed to update profile picture reference"));
    }

    if let Some(old_picture_id) = &target_picture_id
        && let Err(err) = images_db::delete_profile_picture(&state.mongodb, old_picture_id).await
    {
        tracing::error!("Failed to delete old profile picture: {:?}", err);
    }

    state.user_cache.invalidate(&target_user_id).await;

    Ok(Json(
        json!({ "profile_picture_id": file_id, "profile_picture_url": format!("/api/auth/profile-picture/{}", file_id) }),
    ))
}

#[utoipa::path(
    get,
    path = "/auth/profile-picture/{file_id}",
    responses(
        (status = 200, description = "Profile picture", content_type = "image/webp"),
        (status = 404, description = "Picture not found", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "User"
)]
pub async fn get_profile_picture(
    AnyAuthUser(_claims, _user): AnyAuthUser,
    State(state): State<AppState>,
    axum::extract::Path(file_id): axum::extract::Path<String>,
) -> Result<
    (
        StatusCode,
        [(header::HeaderName, header::HeaderValue); 1],
        Vec<u8>,
    ),
    crate::error::AppError,
> {
    if let Some((content_type, data)) = images_db::get_profile_picture(&state.mongodb, &file_id)
        .await
        .map_err(|e| err_not_found(&e.to_string()))?
    {
        let header_value = header::HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream"));
        return Ok((StatusCode::OK, [(header::CONTENT_TYPE, header_value)], data));
    }

    Err(err_not_found("Profile picture not found"))
}

#[utoipa::path(
    delete,
    path = "/auth/profile-picture",
    params(AdminProfilePictureQuery),
    responses(
        (status = 200, description = "Profile picture deleted successfully"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "User"
)]
pub async fn delete_profile_picture_handler(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    Query(query): Query<AdminProfilePictureQuery>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let (target_user_id, target_picture_id) = if query.email.as_deref().unwrap_or("").is_empty() {
        (user.id.clone(), user.profile_picture_id.clone())
    } else {
        if !user.can_manage_branch() {
            return Err(err_forbidden("Only managers can update member profiles"));
        }

        let target =
            UserService::get_user_by_email(&state.postgres, query.email.as_deref().unwrap_or(""))
                .await?;

        if user.is_company_manager() && user.company_id != target.company_id {
            return Err(err_forbidden("Cannot update users from other companies"));
        }

        if user.is_branch_manager() && user.branch_id != target.branch_id {
            return Err(err_forbidden(
                "Branch managers can only manage users in their branch",
            ));
        }

        if target.is_logsmart_admin() && !user.is_logsmart_admin() {
            return Err(err_forbidden("Cannot modify LogSmart internal admin users"));
        }

        (target.id, target.profile_picture_id)
    };

    if let Err(err) =
        db::update_user_profile_picture_id(&state.postgres, &target_user_id, None).await
    {
        tracing::error!("Failed to delete profile picture: {:?}", err);
        return Err(err_internal("Failed to delete profile picture"));
    }

    if let Some(picture_id) = &target_picture_id
        && let Err(err) = images_db::delete_profile_picture(&state.mongodb, picture_id).await
    {
        tracing::error!("Failed to delete profile picture from Mongo: {:?}", err);
        if let Err(rollback_err) =
            db::update_user_profile_picture_id(&state.postgres, &target_user_id, Some(picture_id))
                .await
        {
            tracing::error!(
                "Failed to rollback profile picture reference: {:?}",
                rollback_err
            );
        }
        return Err(err_internal("Failed to delete profile picture"));
    }

    state.user_cache.invalidate(&target_user_id).await;

    Ok(Json(
        json!({ "message": "Profile picture deleted successfully" }),
    ))
}
