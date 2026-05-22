use crate::{
    AppState, db,
    dto::{CompanyResponse, ErrorResponse, ExportResponse, UpdateCompanyRequest},
    exports_db, images_db, logs_db,
    middleware::{AuditRequestContext, ManageCompanyUser},
    utils::{AuditLogger, err_bad_request, err_forbidden, err_internal, err_not_found},
};
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderValue, StatusCode, header},
};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::json;

// Regex to validate export filename: allow only alphanumeric, dots, underscores, and hyphens
static VALID_FILENAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z0-9._-]+$").expect("Invalid regex pattern"));

#[utoipa::path(
    post,
    path = "/companies/{company_id}/logo",
    request_body = Vec<u8>,
    responses(
        (status = 200, description = "Logo uploaded successfully", body = serde_json::Value),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn upload_company_logo(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
    body: Bytes,
) -> Result<impl axum::response::IntoResponse, crate::error::AppError> {
    if user.company_id.as_ref() != Some(&company_id) {
        return Err(crate::error::AppError::Forbidden("You can only manage your own company's logo".to_string()));
    }

    let file_id = crate::services::CompanyService::upload_company_logo(
        &state.postgres,
        &state.mongodb,
        &company_id,
        body.to_vec(),
    )
    .await?;

    AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Uploaded company logo for company {}", company_id),
        crate::audit_ctx!(&audit_ctx, actor: &user),
    )
    .await;

    Ok(Json(json!({
        "logo_id": file_id,
        "logo_url": format!("/api/companies/{}/logo", company_id)
    })))
}

#[utoipa::path(
    get,
    path = "/companies/{company_id}/logo",
    responses(
        (status = 200, description = "Company logo", content_type = "image/webp"),
        (status = 404, description = "Logo not found", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn get_company_logo(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
) -> Result<
    (
        StatusCode,
        [(header::HeaderName, header::HeaderValue); 1],
        Vec<u8>,
    ),
    crate::error::AppError,
> {
    if !user.is_logsmart_admin() && user.company_id.as_ref() != Some(&company_id) {
        return Err(err_forbidden("You can only view your own company's logo"));
    }

    let company = db::get_company_by_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company: {:?}", e);
            err_internal("Database error")
        })?
        .ok_or_else(|| err_not_found("Company not found"))?;

    let logo_id = company
        .logo_id
        .ok_or_else(|| err_not_found("No logo set for this company"))?;

    if let Some((content_type, data)) = images_db::get_company_logo(&state.mongodb, &logo_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get company logo: {:?}", e);
            err_internal("Failed to get logo")
        })?
    {
        let content_type_header = header::HeaderValue::from_str(&content_type)
            .unwrap_or(header::HeaderValue::from_static("application/octet-stream"));
        Ok((
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type_header)],
            data,
        ))
    } else {
        Err(err_not_found("Logo file not found in storage"))
    }
}

#[utoipa::path(
    delete,
    path = "/companies/{company_id}/logo",
    responses(
        (status = 200, description = "Logo deleted successfully", body = serde_json::Value),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn delete_company_logo(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    if user.company_id.as_ref() != Some(&company_id) {
        return Err(err_forbidden("You can only manage your own company's logo"));
    }

    let company = db::get_company_by_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company: {:?}", e);
            err_internal("Database error")
        })?
        .ok_or_else(|| err_not_found("Company not found"))?;

    let Some(logo_id) = company.logo_id else {
        return Err(err_not_found("No logo to delete"));
    };

    db::update_company_logo_id(&state.postgres, &company_id, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update company logo: {:?}", e);
            err_internal("Failed to delete logo reference")
        })?;

    if let Err(err) = images_db::delete_company_logo(&state.mongodb, &logo_id).await {
        tracing::error!("Failed to delete company logo from storage: {:?}", err);
        let _ = db::update_company_logo_id(&state.postgres, &company_id, Some(&logo_id)).await;
        return Err(err_internal("Failed to delete logo"));
    }

    AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Deleted company logo for company {}", company_id),
        crate::audit_ctx!(&audit_ctx, actor: &user),
    )
    .await;

    Ok(Json(json!({ "message": "Logo deleted successfully" })))
}

#[utoipa::path(
    get,
    path = "/companies/{company_id}",
    responses(
        (status = 200, description = "Company details", body = CompanyResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn get_company(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
) -> Result<Json<CompanyResponse>, crate::error::AppError> {
    if user.company_id.as_ref() != Some(&company_id) {
        return Err(err_forbidden("You can only view your own company"));
    }

    let company = db::get_company_by_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company: {:?}", e);
            err_internal("Database error")
        })?
        .ok_or_else(|| err_not_found("Company not found"))?;

    Ok(Json(CompanyResponse::from(company)))
}

#[utoipa::path(
    put,
    path = "/companies/{company_id}",
    request_body = UpdateCompanyRequest,
    responses(
        (status = 200, description = "Company updated successfully", body = CompanyResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn update_company(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
    Json(payload): Json<UpdateCompanyRequest>,
) -> Result<Json<CompanyResponse>, crate::error::AppError> {
    if user.company_id.as_ref() != Some(&company_id) {
        return Err(err_forbidden("You can only update your own company"));
    }

    if payload.name.trim().is_empty() {
        return Err(err_bad_request("Company name cannot be empty"));
    }

    if payload.address.trim().is_empty() {
        return Err(err_bad_request("Company address cannot be empty"));
    }

    let company = db::update_company(
        &state.postgres,
        &company_id,
        &payload.name,
        &payload.address,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to update company: {:?}", e);
        err_internal("Database error")
    })?;

    // Invalidate cache for all users in the company, including soft-deleted ones
    let all_users = match db::get_all_users_by_company_id(&state.postgres, &company_id).await {
        Ok(users) => users,
        Err(e) => {
            tracing::warn!("Failed to fetch users for cache invalidation: {e:?}");
            Vec::new()
        }
    };

    for user in &all_users {
        state.user_cache.invalidate(&user.id).await;
    }

    AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Updated company profile for company {}", company_id),
        crate::audit_ctx!(&audit_ctx, actor: &user),
    )
    .await;

    Ok(Json(CompanyResponse::from(company)))
}

#[utoipa::path(
    post,
    path = "/companies/{company_id}/export",
    responses(
        (status = 200, description = "Export started, download link will be emailed", body = ExportResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
        (status = 429, description = "Rate limited - one export per week", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn export_company_data(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
) -> Result<Json<ExportResponse>, crate::error::AppError> {
    if user.company_id.as_ref() != Some(&company_id) {
        return Err(err_forbidden("You can only export your own company data"));
    }

    let company = db::get_company_by_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company: {:?}", e);
            err_internal("Database error")
        })?
        .ok_or_else(|| err_not_found("Company not found"))?;

    if !state.rate_limit.check_export(&company_id) {
        return Err(crate::error::AppError::TooManyRequests("You can only export once per week. Please try again later.".to_string()));
    }

    let company_email = user.email.clone();
    let company_name = company.name.clone();

    let log_templates = logs_db::get_templates_by_company(&state.mongodb, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch log templates for export: {:?}", e);
            err_internal("Failed to fetch log templates")
        })?;

    let log_entries = logs_db::get_company_log_entries(&state.mongodb, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch log entries for export: {:?}", e);
            err_internal("Failed to fetch log entries")
        })?;

    let exported_at = chrono::Utc::now();

    let templates_json = serde_json::to_string_pretty(&log_templates).unwrap_or_default();
    let entries_json = serde_json::to_string_pretty(&log_entries).unwrap_or_default();

    let zip_data = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            zip.start_file("templates.json", options)?;
            zip.write_all(templates_json.as_bytes())?;

            zip.start_file("log_entries.json", options)?;
            zip.write_all(entries_json.as_bytes())?;

            zip.finish()?;
        }
        Ok(buf)
    })
    .await
    .map_err(|e| {
        tracing::error!("ZIP generation failed: {:?}", e);
        err_internal("Failed to generate export")
    })?
    .map_err(|e| {
        tracing::error!("ZIP generation error: {:?}", e);
        err_internal("Failed to generate export")
    })?;

    let filename = exports_db::save_export(&company_id, &zip_data)
        .await
        .map_err(|e| {
            tracing::error!("Failed to save export file: {:?}", e);
            err_internal("Failed to save export")
        })?;

    db::mark_company_data_exported(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to mark company data as exported: {:?}", e);
            err_internal("Failed to mark data as exported")
        })?;

    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let export_company_id = company_id.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::email::send_export_ready_notification(
            &company_email,
            &company_name,
            &export_company_id,
            &filename,
            &frontend_url,
        )
        .await
        {
            tracing::error!("Failed to send export notification email: {:?}", e);
        }
    });

    AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Requested company export for company {}", company_id),
        crate::audit_ctx!(&audit_ctx, actor: &user),
    )
    .await;

    Ok(Json(ExportResponse {
        message: "Export complete. A download link has been sent to your email.".to_string(),
        exported_at,
    }))
}

#[utoipa::path(
    get,
    path = "/companies/{company_id}/export/download/{filename}",
    params(
        ("company_id" = String, Path, description = "Company ID"),
        ("filename" = String, Path, description = "Export filename"),
    ),
    responses(
        (status = 200, description = "Export file", content_type = "application/zip"),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Export not found", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn download_export(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path((company_id, filename)): axum::extract::Path<(String, String)>,
) -> Result<axum::response::Response, crate::error::AppError> {
    if user.company_id.as_ref() != Some(&company_id) {
        return Err(err_forbidden(
            "You can only download your own company exports",
        ));
    }

    // Validate basic structure: must end with .zip and start with company_id_
    if !filename.ends_with(".zip") || !filename.starts_with(&format!("{company_id}_")) {
        return Err(err_bad_request("Invalid export filename"));
    }

    // Validate filename against strict allow-list (alphanumeric, dots, underscores, hyphens)
    if !VALID_FILENAME_REGEX.is_match(&filename) {
        return Err(err_bad_request("Invalid export filename"));
    }

    let data = exports_db::get_export(&company_id, &filename)
        .await
        .map_err(|e| {
            tracing::error!("Failed to read export file: {:?}", e);
            err_internal("Failed to read export")
        })?
        .ok_or_else(|| err_not_found("Export file not found or expired"))?;

    let content_length = data.len().to_string();

    // Build Content-Disposition header safely
    let disposition_value = format!("attachment; filename=\"{filename}\"");
    let disposition_header = HeaderValue::from_str(&disposition_value).map_err(|_| {
        tracing::error!("Invalid filename for header: {:?}", filename);
        err_bad_request("Invalid export filename")
    })?;

    let response = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_LENGTH, content_length)
        .header(header::CONTENT_DISPOSITION, disposition_header)
        .body(axum::body::Body::from(data))
        .map_err(|_| {
            tracing::error!("Failed to build response");
            err_internal("Failed to build response")
        })?;

    let _ = AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Downloaded export {} for company {}", filename, company_id),
        crate::audit_ctx!(&audit_ctx, actor: &user),
    )
    .await;

    Ok(response)
}

#[utoipa::path(
    delete,
    path = "/companies/{company_id}",
    responses(
        (status = 200, description = "Company deletion requested", body = serde_json::Value),
        (status = 400, description = "Data must be exported first", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Company"
)]
pub async fn delete_company(
    ManageCompanyUser(_claims, user): ManageCompanyUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    if user.company_id.as_ref() != Some(&company_id) {
        return Err(err_forbidden("You can only delete your own company"));
    }

    let company = db::get_company_by_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company: {:?}", e);
            err_internal("Database error")
        })?
        .ok_or_else(|| err_not_found("Company not found"))?;

    if let Some(exported_at) = company.data_exported_at {
        if exported_at < chrono::Utc::now() - chrono::Duration::hours(6) {
            return Err(err_bad_request(
                "Data export has expired. Please export company data again before requesting deletion.",
            ));
        }
    } else {
        return Err(err_bad_request(
            "You must export company data before requesting deletion",
        ));
    }

    if company.deletion_requested_at.is_some()
        && company.deletion_requested_at.unwrap() >= chrono::Utc::now() - chrono::Duration::hours(6)
    {
        return Err(err_bad_request(
            "A deletion request is already pending. Please check your email to confirm.",
        ));
    }

    let company_email = user.email.clone();
    let company_name = company.name.clone();

    let updated_company =
        db::request_company_deletion(&state.postgres, &company_id, &company_email)
            .await
            .map_err(|e| {
                tracing::error!("Failed to request company deletion: {:?}", e);
                err_internal("Failed to request company deletion")
            })?;

    let deletion_token = updated_company.deletion_token.unwrap_or_default();

    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let delete_company_id = company_id.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::email::send_company_deletion_request(
            &company_email,
            &company_name,
            &delete_company_id,
            &deletion_token,
            &frontend_url,
        )
        .await
        {
            tracing::error!("Failed to send company deletion request email: {:?}", e);
        }
    });

    AuditLogger::log_admin_action(
        &state.postgres,
        user.id.clone(),
        format!("Requested company deletion for company {}", company_id),
        crate::audit_ctx!(&audit_ctx, actor: &user),
    )
    .await;

    Ok(Json(
        json!({ "message": "Deletion requested. Please check your email to confirm." }),
    ))
}

#[utoipa::path(
    get,
    path = "/companies/{company_id}/validate-deletion-token",
    responses(
        (status = 200, description = "Token valid, returns company name", body = serde_json::Value),
        (status = 400, description = "Invalid or expired token", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
    ),
    tag = "Company"
)]
pub async fn validate_company_deletion_token(
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
    axum::extract::Query(token): axum::extract::Query<serde_json::Value>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let token = token.get("token").and_then(|t| t.as_str()).unwrap_or("");

    if token.is_empty() {
        return Err(err_bad_request("Confirmation token is required"));
    }

    let company = db::get_company_by_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company: {:?}", e);
            err_internal("Database error")
        })?
        .ok_or_else(|| err_not_found("Company not found"))?;

    if company.deletion_token.as_deref() != Some(token) {
        AuditLogger::log(
            &state.postgres,
            "company_deletion_token_invalid",
            None,
            None,
            crate::audit_ctx!(&audit_ctx, company_id: Some(company_id.clone())),
            Some("Invalid or expired confirmation token".to_string()),
            false,
        )
        .await;
        return Err(err_bad_request("Invalid or expired confirmation token"));
    }

    if let Some(requested_at) = company.deletion_requested_at
        && requested_at < chrono::Utc::now() - chrono::Duration::hours(6)
    {
        AuditLogger::log(
            &state.postgres,
            "company_deletion_token_invalid",
            None,
            None,
            crate::audit_ctx!(&audit_ctx, company_id: Some(company_id.clone())),
            Some("Confirmation token expired".to_string()),
            false,
        )
        .await;
        return Err(err_bad_request(
            "Confirmation token has expired. Please request a new deletion link.",
        ));
    }

    AuditLogger::log(
        &state.postgres,
        "company_deletion_token_validated",
        None,
        None,
        crate::audit_ctx!(&audit_ctx, company_id: Some(company_id.clone())),
        Some("Company deletion token validated".to_string()),
        true,
    )
    .await;

    Ok(Json(json!({ "companyName": company.name })))
}

#[utoipa::path(
    post,
    path = "/companies/{company_id}/confirm-deletion",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Company deletion confirmed", body = serde_json::Value),
        (status = 400, description = "Invalid or expired token", body = ErrorResponse),
        (status = 404, description = "Company not found", body = ErrorResponse),
    ),
    tag = "Company"
)]
pub async fn confirm_company_deletion(
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    axum::extract::Path(company_id): axum::extract::Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let token = payload.get("token").and_then(|t| t.as_str()).unwrap_or("");

    if token.is_empty() {
        return Err(err_bad_request("Confirmation token is required"));
    }

    let company = db::get_company_by_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Database error fetching company: {:?}", e);
            err_internal("Database error")
        })?
        .ok_or_else(|| err_not_found("Company not found"))?;

    if company.deletion_token.as_deref() != Some(token) {
        return Err(err_bad_request("Invalid or expired confirmation token"));
    }

    if let Some(requested_at) = company.deletion_requested_at
        && requested_at < chrono::Utc::now() - chrono::Duration::hours(6)
    {
        return Err(err_bad_request(
            "Confirmation token has expired. Please request a new deletion link.",
        ));
    }

    let updated_company =
        match db::confirm_company_deletion(&state.postgres, &company_id, token).await {
            Ok(Some(company)) => company,
            Ok(None) => return Err(err_bad_request("Invalid or expired confirmation token")),
            Err(e) => {
                tracing::error!("Failed to confirm company deletion: {:?}", e);
                return Err(err_bad_request("Invalid or expired confirmation token"));
            }
        };

    db::soft_delete_users_by_company_id(&state.postgres, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to soft delete company users: {:?}", e);
            err_internal("Failed to delete company users")
        })?;

    // Invalidate cache for all users in the company, including soft-deleted ones
    // Note: This runs after deletion is confirmed, so get_all_users_by_company_id
    // must not filter by deleted_at to work correctly
    let all_users = db::get_all_users_by_company_id(&state.postgres, &company_id)
        .await
        .unwrap_or_default();

    for user in &all_users {
        state.user_cache.invalidate(&user.id).await;
    }

    let user_email = updated_company.deletion_requested_by_email.clone();
    let company_name = updated_company.name.clone();

    tokio::spawn(async move {
        if let Err(e) = crate::email::send_company_deleted_notification(&company_name).await {
            tracing::error!("Failed to send company deletion notification: {:?}", e);
        }
        if let Some(email) = user_email
            && let Err(e) =
                crate::email::send_user_company_deleted_notification(&email, &company_name).await
        {
            tracing::error!("Failed to send user company deletion notification: {:?}", e);
        }
    });

    AuditLogger::log(
        &state.postgres,
        "company_deletion_confirmed",
        None,
        updated_company.deletion_requested_by_email.clone(),
        crate::audit_ctx!(&audit_ctx, company_id: Some(company_id.clone())),
        Some(format!("Confirmed deletion for company {}", company_id)),
        true,
    )
    .await;

    Ok(Json(
        json!({ "message": "Company has been deleted. Data will be retained for 30 days." }),
    ))
}
