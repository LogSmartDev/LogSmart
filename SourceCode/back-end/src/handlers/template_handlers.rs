use crate::{
    AppState,
    dto::{
        AddTemplateRequest, AddTemplateResponse, DeleteTemplateRequest, DeleteTemplateResponse,
        ErrorResponse, GetAllTemplatesResponse, GetTemplateRequest, GetTemplateResponse,
        GetTemplateVersionsResponse, RenameTemplateRequest, RenameTemplateResponse,
        RestoreTemplateVersionRequest, TemplateInfo, UpdateTemplateRequest, UpdateTemplateResponse,
    },
    middleware::{AnyAuthUser, BranchManagerUser, ReadBranchUser},
    services,
    utils::{err_validation, friendly_validation_errors},
};
use axum::{
    Json,
    extract::{Query, State},
};
use validator::Validate;

#[utoipa::path(
    post,
    path = "/logs/templates",
    request_body = AddTemplateRequest,
    responses(
        (status = 200, description = "Template added successfully", body = AddTemplateResponse),
        (status = 401, description = "Invalid or expired token", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Adds a new log template for the current company.
pub async fn add_template(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    Json(payload): Json<AddTemplateRequest>,
) -> Result<Json<AddTemplateResponse>, crate::error::AppError> {
    // Validate request payload
    payload.validate().map_err(|e| {
        let (msg, fields) = friendly_validation_errors(&e);
        err_validation(&msg, fields)
    })?;

    // Branch managers can only create templates for their own branch
    if user.is_branch_manager() {
        if payload.branch_id.is_none() {
            return Err(crate::error::AppError::Forbidden(
                "Branch managers cannot create company-wide templates".to_string(),
            ));
        }
        if payload.branch_id != user.branch_id {
            return Err(crate::error::AppError::Forbidden(
                "Branch managers can only create templates for their own branch".to_string(),
            ));
        }
    }

    let company_id = user
        .company_id
        .clone()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    services::TemplateService::create_template(
        &state,
        &company_id,
        payload.template_name,
        payload.template_layout,
        payload.schedule,
        &user.id,
        payload.branch_id,
    )
    .await?;

    Ok(Json(AddTemplateResponse {
        message: "Template added successfully.".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/logs/templates",
    params(
        ("template_name"=String, Query, description = "Name of the template to retrieve", example = "ErrorLog" )
    ),
    responses(
        (status = 200, description = "Template retrieved successfully", body = GetTemplateResponse),
        (status = 401, description = "Invalid or expired token", body = ErrorResponse),
        (status = 404, description = "Template not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Retrieves a specific log template by name.
pub async fn get_template(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    Query(payload): Query<GetTemplateRequest>,
) -> Result<Json<GetTemplateResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .as_deref()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    let (template_name, template_layout, version, version_name, branch_id) =
        services::TemplateService::get_template(&state, company_id, &payload.template_name).await?;

    if let Some(branch) = branch_id.clone() {
        // If the template is branch-specific, check if the user has access to that branch
        if let Some(user_branch_id) = &user.branch_id {
            if &branch != user_branch_id && !user.can_manage_company() {
                return Err(crate::error::AppError::Forbidden(
                    "User does not have access to this template".to_string(),
                ));
            }
        } else if !user.can_manage_company() {
            // If the user is not associated with any branch and is not a company manager, deny access
            return Err(crate::error::AppError::Forbidden(
                "User does not have access to this template".to_string(),
            ));
        }
    }

    Ok(Json(GetTemplateResponse {
        template_name,
        template_layout,
        version,
        version_name,
        branch_id,
    }))
}

#[utoipa::path(
    get,
    path = "/logs/templates/all",
    responses(
        (status = 200, description = "All templates retrieved successfully", body = GetAllTemplatesResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Retrieves all log templates for the current company/branch.
pub async fn get_all_templates(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
) -> Result<Json<GetAllTemplatesResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .as_deref()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    let templates = services::TemplateService::get_all_templates(
        &state,
        company_id,
        if user.is_branch_manager() {
            user.branch_id.as_deref()
        } else {
            None
        },
    )
    .await?;

    let response_templates = templates
        .into_iter()
        .map(
            |(name, created_at, updated_at, user_id, schedule)| TemplateInfo {
                template_name: name,
                created_at: created_at.to_string(),
                updated_at: updated_at.to_string(),
                created_by: user_id,
                schedule,
            },
        )
        .collect();

    Ok(Json(GetAllTemplatesResponse {
        templates: response_templates,
    }))
}

#[utoipa::path(
    put,
    path = "/logs/templates/update",
    request_body = UpdateTemplateRequest,
    responses(
        (status = 200, description = "Template updated successfully", body = UpdateTemplateResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Updates an existing log template.
pub async fn update_template(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    Json(payload): Json<UpdateTemplateRequest>,
) -> Result<Json<UpdateTemplateResponse>, crate::error::AppError> {
    // Validate request payload
    payload.validate().map_err(|e| {
        let (msg, fields) = friendly_validation_errors(&e);
        err_validation(&msg, fields)
    })?;

    services::TemplateService::update_template(
        &state,
        &payload.template_name,
        payload.template_layout.as_ref(),
        payload.schedule.as_ref(),
        &user,
        payload.version_name.clone(),
        payload.branch_id.as_ref().map(|branch| {
            if branch == "company" {
                None
            } else {
                Some(branch.as_str())
            }
        }),
    )
    .await?;
    Ok(Json(UpdateTemplateResponse {
        message: "Template updated successfully.".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/logs/templates/versions",
    params(
        ("template_name"=String, Query, description = "Name of the template to retrieve versions for", example = "ErrorLog")
    ),
    responses(
        (status = 200, description = "Versions retrieved successfully", body = GetTemplateVersionsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Retrieves the version history of a log template.
pub async fn get_template_versions(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    Query(payload): Query<GetTemplateRequest>,
) -> Result<Json<crate::dto::GetTemplateVersionsResponse>, crate::error::AppError> {
    let versions =
        services::TemplateService::get_versions(&state, &payload.template_name, &user).await?;

    let version_infos = versions
        .into_iter()
        .map(|v| crate::dto::TemplateVersionInfo {
            version: v.version,
            version_name: v.version_name,
            created_at: v.created_at.to_string(),
            created_by: v.created_by.to_string(),
        })
        .collect();

    Ok(Json(crate::dto::GetTemplateVersionsResponse {
        versions: version_infos,
    }))
}

#[utoipa::path(
    post,
    path = "/logs/templates/versions/restore",
    params(
        ("template_name"=String, Query, description = "Name of the template to restore", example = "ErrorLog")
    ),
    request_body = RestoreTemplateVersionRequest,
    responses(
        (status = 200, description = "Template restored successfully", body = UpdateTemplateResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Restores a specific version of a log template.
pub async fn restore_template_version(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    Query(query): Query<GetTemplateRequest>,
    Json(payload): Json<crate::dto::RestoreTemplateVersionRequest>,
) -> Result<Json<UpdateTemplateResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .as_ref()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    services::TemplateService::restore_version(
        &state,
        company_id,
        &query.template_name,
        payload.version,
        &user,
    )
    .await?;

    Ok(Json(UpdateTemplateResponse {
        message: format!("Template restored to version {}", payload.version),
    }))
}

#[utoipa::path(
    put,
    path = "/logs/templates/rename",
    request_body = RenameTemplateRequest,
    responses(
        (status = 200, description = "Template renamed successfully", body = RenameTemplateResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Renames an existing log template.
pub async fn rename_template(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    Json(payload): Json<RenameTemplateRequest>,
) -> Result<Json<RenameTemplateResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .as_ref()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    services::TemplateService::rename_template(
        &state,
        company_id,
        &payload.old_template_name,
        &payload.new_template_name,
        user.branch_id.as_deref(),
        &user.role,
    )
    .await?;
    Ok(Json(RenameTemplateResponse {
        message: "Template renamed successfully.".to_string(),
    }))
}

#[utoipa::path(
    delete,
    path = "/logs/templates",
    params(
        DeleteTemplateRequest
    ),
    responses(
        (status = 200, description = "Template deleted successfully", body = DeleteTemplateResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Templates"
)]
/// Deletes a specific log template.
pub async fn delete_template(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    Query(payload): Query<DeleteTemplateRequest>,
) -> Result<Json<DeleteTemplateResponse>, crate::error::AppError> {
    let company_id = user
        .company_id
        .as_ref()
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

    services::TemplateService::delete_template(
        &state,
        company_id,
        &payload.template_name,
        user.branch_id.as_deref(),
        &user.role,
    )
    .await?;
    Ok(Json(DeleteTemplateResponse {
        message: "Template deleted successfully.".to_string(),
    }))
}
