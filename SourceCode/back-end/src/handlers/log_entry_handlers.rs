use std::collections::{HashMap, HashSet};

use crate::{
    AppState,
    dto::{
        CreateLogEntryRequest, CreateLogEntryResponse, CreateReportRunRequest,
        CreateReportRunResponse, DeleteReportRunResponse, DueFormInfo, DueFormsResponse,
        ErrorResponse, ListLogEntriesResponse, ListReportRunsResponse, LogEntryResponse,
        ReportRunResponse, SubmitLogEntryResponse, UpdateLogEntryRequest, UseReportRunResponse,
    },
    logs_db::{self, LogStatus},
    middleware::{AnyAuthUser, BranchManagerUser, ReadBranchUser},
    services,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde_json::json;
use uuid::Uuid;

#[utoipa::path(
    get,
    path = "/logs/entries/due",
    params(("max" = u32, Query, description = "Optional max number of due forms to return, default 20")),
    responses(
        (status = 200, description = "Due forms retrieved successfully", body = DueFormsResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Lists all log forms that are due today for the current user's company.
///
/// # Errors
/// Returns an error if the user is not authorized or if the query fails.
pub async fn list_due_forms_today(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<DueFormsResponse>, crate::error::AppError> {
    let max = params
        .get("max")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(20);

    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let templates =
        services::LogEntryService::list_due_forms(&state, &company_id, user.branch_id.as_deref())
            .await
            ?;

    let now = chrono::Utc::now();

    // First, fetch latest submitted entries for ALL templates to get actual last periods
    let all_template_names: Vec<String> =
        templates.iter().map(|t| t.template_name.clone()).collect();

    let latest_submitted_entries = logs_db::get_latest_submitted_entries_batch(
        &state.mongodb,
        &user.id,
        &company_id,
        &all_template_names,
    )
    .await
    .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to retrieve due forms".to_string())
        })?;

    // Now compute missed periods using actual last submitted periods
    let mut template_due_counts: HashMap<String, usize> = HashMap::new();
    let mut needed_templates: Vec<&logs_db::TemplateDocument> = Vec::new();
    let mut accumulated = 0usize;

    for template in &templates {
        let last_submitted = latest_submitted_entries.get(&template.template_name);
        let last_period = last_submitted.as_ref().map(|e| e.period.as_str());
        let created_at = template.created_at.to_rfc3339();
        let missed_periods =
            logs_db::get_missed_periods(&template.schedule, last_period, Some(&created_at));
        let missed_count = missed_periods.len();
        let due_today = if logs_db::is_form_due_today(&template.schedule) {
            1
        } else {
            0
        };
        let total_due = missed_count + due_today;

        if accumulated < max as usize {
            needed_templates.push(template);
            template_due_counts.insert(template.template_name.clone(), total_due);
            accumulated += total_due;
        }
    }

    let needed_template_names: Vec<String> = needed_templates
        .iter()
        .map(|t| t.template_name.clone())
        .collect();

    let mut all_missed_periods: Vec<String> = Vec::new();
    let mut template_missed_periods: HashMap<String, Vec<String>> = HashMap::new();

    for template in &needed_templates {
        let last_submitted = latest_submitted_entries.get(&template.template_name);
        let last_period = last_submitted.as_ref().map(|e| e.period.as_str());
        let created_at = template.created_at.to_rfc3339();
        let mut missed_periods =
            logs_db::get_missed_periods(&template.schedule, last_period, Some(&created_at));
        missed_periods.reverse();
        all_missed_periods.extend(missed_periods.clone());
        template_missed_periods.insert(template.template_name.clone(), missed_periods);
    }

    let periods_with_entries = logs_db::get_periods_with_entries_batch(
        &state.mongodb,
        &company_id,
        &needed_template_names,
        &all_missed_periods,
    )
    .await
    .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to retrieve due forms".to_string())
        })?;

    let mut due_forms = Vec::new();
    let mut seen_forms: HashSet<(String, String)> = HashSet::new();

    for template in &needed_templates {
        let last_submitted = latest_submitted_entries
            .get(&template.template_name)
            .cloned();
        let missed_periods = template_missed_periods.get(&template.template_name);
        let periods_with_entries_map = periods_with_entries.get(&template.template_name);

        if let Some(missed_periods) = missed_periods {
            for period in missed_periods {
                if let Some(periods_with_entries_map) = periods_with_entries_map
                    && periods_with_entries_map.contains(period)
                {
                    continue;
                }

                let form_key = (template.template_name.clone(), period.clone());
                if seen_forms.contains(&form_key) {
                    continue;
                }
                seen_forms.insert(form_key);

                if due_forms.len() >= max as usize {
                    break;
                }

                let status =
                    logs_db::get_availability_status_for_period(&template.schedule, period, now);

                let processed_layout = logs_db::process_template_layout_with_period_string(
                    &template.template_layout,
                    period,
                );

                let available_from =
                    logs_db::get_available_from_datetime(&template.schedule, period);
                let due_at = logs_db::get_due_at_datetime(&template.schedule, period);

                due_forms.push(DueFormInfo {
                    template_name: template.template_name.clone(),
                    template_layout: processed_layout,
                    last_submitted: last_submitted
                        .as_ref()
                        .and_then(|e| e.submitted_at.map(|ts| ts.to_rfc3339())),
                    period: period.clone(),
                    status: Some(LogStatus::Overdue.as_str().to_string()),
                    availability_status: status.as_str().to_string(),
                    available_from,
                    due_at,
                });
            }
        }

        if due_forms.len() >= max as usize {
            break;
        }

        // Batch-check which templates are due today and have no submission
        let due_today_templates: Vec<&logs_db::TemplateDocument> = needed_templates
            .iter()
            .filter(|t| logs_db::is_form_due_today(&t.schedule))
            .copied()
            .collect();

        if !due_today_templates.is_empty() {
            let due_template_names: Vec<String> = due_today_templates
                .iter()
                .map(|t| t.template_name.clone())
                .collect();

            // Single batch query for all submission checks
            let templates_with_freq: Vec<(&str, &logs_db::Frequency)> = due_today_templates
                .iter()
                .map(|t| (t.template_name.as_str(), &t.schedule.frequency))
                .collect();
            let submitted_map = logs_db::has_submitted_entries_batch(
                &state.mongodb,
                &company_id,
                &templates_with_freq,
            )
            .await
            .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to retrieve due forms".to_string())
        })?;

            // Single batch query for all draft entries
            let draft_map = logs_db::get_draft_entries_batch(
                &state.mongodb,
                &user.id,
                &company_id,
                &due_template_names,
            )
            .await
            .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to retrieve due forms".to_string())
        })?;

            for template in due_today_templates {
                let has_submitted = submitted_map.get(&template.template_name).copied().unwrap_or(false);

                if !has_submitted {
                    let draft_entry = draft_map.get(&template.template_name).and_then(|e| e.clone());

                    let last_submitted = latest_submitted_entries
                        .get(&template.template_name)
                        .cloned();

                    let processed_layout = logs_db::process_template_layout_with_period(
                        &template.template_layout,
                        &template.schedule.frequency,
                    );

                    let period = logs_db::format_period_for_frequency(&template.schedule.frequency);
                    let available_from =
                        logs_db::get_available_from_datetime(&template.schedule, &period);
                    let due_at = logs_db::get_due_at_datetime(&template.schedule, &period);

                    let form_key = (template.template_name.clone(), period.clone());
                    if seen_forms.contains(&form_key) {
                        continue;
                    }
                    seen_forms.insert(form_key);

                    let status =
                        logs_db::get_availability_status_for_period(&template.schedule, &period, now);

                    let derived_draft_status = draft_entry.as_ref().map(|e| {
                        logs_db::derive_log_status(e.status, &template.schedule, &period, now)
                            .0
                            .as_str()
                            .to_string()
                    });

                    due_forms.push(DueFormInfo {
                        template_name: template.template_name.clone(),
                        template_layout: processed_layout,
                        last_submitted: last_submitted
                            .as_ref()
                            .and_then(|e| e.submitted_at.map(|ts| ts.to_rfc3339())),
                        period,
                        status: derived_draft_status,
                        availability_status: status.as_str().to_string(),
                        available_from,
                        due_at,
                    });
                }
            }
        }
    }

    let forms = due_forms;

    Ok(Json(DueFormsResponse { forms }))
}

#[utoipa::path(
    post,
    path = "/logs/entries",
    request_body = CreateLogEntryRequest,
    responses(
        (status = 201, description = "Log entry created successfully", body = CreateLogEntryResponse),
        (status = 400, description = "Invalid request data", body = ErrorResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 404, description = "Template not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Creates a new log entry draft.
///
/// # Errors
/// Returns an error if the template name is empty or if entry creation fails.
pub async fn create_log_entry(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateLogEntryRequest>,
) -> Result<(StatusCode, Json<CreateLogEntryResponse>), crate::error::AppError> {
    if payload.template_name.is_empty() {
        return Err(crate::error::AppError::BadRequest("Template name is required".to_string()));
    }

    let entry_id = services::LogEntryService::create_log_entry(
        &state,
        &user,
        &payload.template_name,
        payload.period.as_deref(),
    )
    .await
    ?;

    Ok((
        StatusCode::CREATED,
        Json(CreateLogEntryResponse {
            id: entry_id,
            message: "Log entry created successfully.".to_string(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/logs/entries/{entry_id}",
    responses(
        (status = 200, description = "Log entry retrieved successfully", body = LogEntryResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 403, description = "Forbidden - entry does not belong to user", body = ErrorResponse),
        (status = 404, description = "Entry not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Retrieves a specific log entry by its ID.
///
/// # Errors
/// Returns an error if the entry is not found or if the user is not authorized.
pub async fn get_log_entry(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
) -> Result<Json<LogEntryResponse>, crate::error::AppError> {
    let entry = services::LogEntryService::get_log_entry(&state, &user, &entry_id)
        .await
        ?;

    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let template = logs_db::get_template_by_name(&state.mongodb, &entry.template_name, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to get template".to_string())
        })?
        .ok_or(crate::error::AppError::NotFound("Template not found".to_string()))?;

    let processed_layout = logs_db::process_template_layout_with_period_string(
        &template.template_layout,
        &entry.period,
    );

    let availability = logs_db::get_availability_status_for_period(
        &template.schedule,
        &entry.period,
        chrono::Utc::now(),
    );

    Ok(Json(LogEntryResponse {
        id: entry.entry_id,
        template_name: entry.template_name,
        template_layout: processed_layout,
        entry_data: entry.entry_data,
        status: entry.status.as_str().to_string(),
        availability_status: availability.as_str().to_string(),
        created_at: entry.created_at.to_rfc3339(),
        updated_at: entry.updated_at.to_rfc3339(),
        submitted_at: entry.submitted_at.map(|ts| ts.to_rfc3339()),
        period: entry.period,
    }))
}

#[utoipa::path(
    put,
    path = "/logs/entries/{entry_id}",
    request_body = UpdateLogEntryRequest,
    responses(
        (status = 200, description = "Log entry updated successfully", body = LogEntryResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 403, description = "Forbidden - entry does not belong to user", body = ErrorResponse),
        (status = 404, description = "Entry not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Updates the data of an existing log entry draft.
///
/// # Errors
/// Returns an error if the entry is not found or if the update fails.
pub async fn update_log_entry(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
    Json(payload): Json<UpdateLogEntryRequest>,
) -> Result<Json<LogEntryResponse>, crate::error::AppError> {
    let updated_entry = services::LogEntryService::update_log_entry(
        &state,
        &user.id,
        &entry_id,
        &payload.entry_data,
    )
    .await
    ?;

    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let template =
        logs_db::get_template_by_name(&state.mongodb, &updated_entry.template_name, &company_id)
            .await
            .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to get template".to_string())
        })?
            .ok_or(crate::error::AppError::NotFound("Template not found".to_string()))?;

    let processed_layout = logs_db::process_template_layout_with_period_string(
        &template.template_layout,
        &updated_entry.period,
    );

    let availability = logs_db::get_availability_status_for_period(
        &template.schedule,
        &updated_entry.period,
        chrono::Utc::now(),
    );

    Ok(Json(LogEntryResponse {
        id: updated_entry.entry_id,
        template_name: updated_entry.template_name,
        template_layout: processed_layout,
        entry_data: updated_entry.entry_data,
        status: updated_entry.status.as_str().to_string(),
        availability_status: availability.as_str().to_string(),
        created_at: updated_entry.created_at.to_rfc3339(),
        updated_at: updated_entry.updated_at.to_rfc3339(),
        submitted_at: updated_entry.submitted_at.map(|ts| ts.to_rfc3339()),
        period: updated_entry.period,
    }))
}

#[utoipa::path(
    post,
    path = "/logs/entries/{entry_id}/submit",
    responses(
        (status = 200, description = "Log entry submitted successfully", body = SubmitLogEntryResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 403, description = "Forbidden - entry does not belong to user", body = ErrorResponse),
        (status = 404, description = "Entry not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Submits a log entry, marking it as final.
///
/// # Errors
/// Returns an error if the entry is not found or if submission fails.
pub async fn submit_log_entry(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
) -> Result<Json<SubmitLogEntryResponse>, crate::error::AppError> {
    services::LogEntryService::submit_log_entry(&state, &user.id, &entry_id)
        .await
        ?;

    Ok(Json(SubmitLogEntryResponse {
        message: "Log entry submitted successfully.".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/logs/entries/{entry_id}/unsubmit",
    responses(
        (status = 200, description = "Log entry returned to draft successfully", body = SubmitLogEntryResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 403, description = "Forbidden - only admins can unsubmit entries", body = ErrorResponse),
        (status = 404, description = "Entry not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Returns a submitted log entry to draft status (admin only).
///
/// # Errors
/// Returns an error if the entry is not found or if the operation fails.
pub async fn unsubmit_log_entry(
    BranchManagerUser(_claims, user): BranchManagerUser,
    State(state): State<AppState>,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
) -> Result<Json<SubmitLogEntryResponse>, crate::error::AppError> {
    services::LogEntryService::unsubmit_log_entry(&state, &user, &entry_id)
        .await
        ?;

    Ok(Json(SubmitLogEntryResponse {
        message: "Log entry returned to draft successfully.".to_string(),
    }))
}

#[utoipa::path(
    delete,
    path = "/logs/entries/{entry_id}",
    responses(
        (status = 200, description = "Log entry deleted successfully", body = serde_json::Value),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 403, description = "Forbidden - entry does not belong to user", body = ErrorResponse),
        (status = 404, description = "Entry not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Deletes a log entry.
///
/// # Errors
/// Returns an error if the user is not authorized or if deletion fails.
pub async fn delete_log_entry(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    services::LogEntryService::delete_log_entry(&state, &user, &entry_id)
        .await
        ?;

    Ok(Json(json!({ "message": "Log entry deleted successfully" })))
}

#[utoipa::path(
    get,
    path = "/logs/admin/entries",
    params(
        ("branch_ids" = String, Query, description = "Optional comma-separated list of branch IDs to filter by (managers only)"),
    ),
    responses(
        (status = 200, description = "Company log entries retrieved successfully", body = ListLogEntriesResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - only managers can view all entries", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Lists all log entries for the company or branch (managers only).
pub async fn list_company_log_entries<S: ::std::hash::BuildHasher>(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String, S>>,
) -> Result<Json<ListLogEntriesResponse>, crate::error::AppError> {
    let company_id = user.company_id.clone().ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    // Parse optional branch_ids parameter (comma-separated)
    let branch_ids_param = params.get("branch_ids");

    let entries = if user.can_manage_company() || user.is_readonly_hq() {
        // Company manager/HQ - check if specific branches requested
        if let Some(branch_ids_str) = branch_ids_param {
            if branch_ids_str.is_empty() {
                logs_db::get_company_log_entries(&state.mongodb, &company_id).await
            } else {
                let branch_ids: Vec<String> = branch_ids_str
                    .split(',')
                    .map(std::string::ToString::to_string)
                    .collect();
                logs_db::get_branches_log_entries(&state.mongodb, &company_id, &branch_ids).await
            }
        } else {
            logs_db::get_company_log_entries(&state.mongodb, &company_id).await
        }
    } else {
        // Branch manager - only their branch
        let branch_id = user.branch_id.as_ref().ok_or(crate::error::AppError::Forbidden("Branch manager has no branch assigned".to_string()))?;
        logs_db::get_branch_log_entries(&state.mongodb, &company_id, branch_id).await
    }
    .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to get log entries".to_string())
        })?;

    let mut response_entries = Vec::new();

    // Batch-fetch all templates once instead of N queries in the loop
    let all_templates = logs_db::get_templates_by_company(&state.mongodb, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to get templates".to_string())
        })?;
    let template_map: HashMap<&str, &logs_db::TemplateDocument> = all_templates
        .iter()
        .map(|t| (t.template_name.as_str(), t))
        .collect();

    for e in entries {
        let template = template_map.get(e.template_name.as_str());

        let (processed_layout, derived_status, availability_status) =
            if let Some(template) = template.copied() {
                let layout = logs_db::process_template_layout_with_period_string(
                    &template.template_layout,
                    &e.period,
                );
                let (status, availability) = logs_db::derive_log_status(
                    e.status,
                    &template.schedule,
                    &e.period,
                    chrono::Utc::now(),
                );
                (layout, status, availability)
            } else {
                (
                    Vec::new(),
                    e.status,
                    logs_db::AvailabilityStatus::NotAvailable,
                )
            };

        response_entries.push(LogEntryResponse {
            id: e.entry_id,
            template_name: e.template_name,
            template_layout: processed_layout,
            entry_data: e.entry_data,
            status: derived_status.as_str().to_string(),
            availability_status: availability_status.as_str().to_string(),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
            submitted_at: e.submitted_at.map(|ts| ts.to_rfc3339()),
            period: e.period,
        });
    }

    Ok(Json(ListLogEntriesResponse {
        entries: response_entries,
    }))
}

#[utoipa::path(
    get,
    path = "/logs/entries",
    responses(
        (status = 200, description = "User log entries retrieved successfully", body = ListLogEntriesResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
#[allow(clippy::implicit_hasher)]
/// Lists all log entries for the current user, optionally filtered by template or status.
///
/// # Errors
/// Returns an error if the user is not authorized or if the query fails.
pub async fn list_user_log_entries(
    AnyAuthUser(_claims, user): AnyAuthUser,
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ListLogEntriesResponse>, crate::error::AppError> {
    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let mut entries =
        services::LogEntryService::get_user_log_entries(&state, &user.id, &company_id)
            .await
            ?;

    if let Some(template_name) = params.get("template_name") {
        entries.retain(|e| e.template_name == *template_name);
    }

    // Batch-fetch all templates once instead of N queries in the loop
    let all_templates = logs_db::get_templates_by_company(&state.mongodb, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to get templates".to_string())
        })?;
    let template_map: HashMap<&str, &logs_db::TemplateDocument> = all_templates
        .iter()
        .map(|t| (t.template_name.as_str(), t))
        .collect();

    let mut response_entries = Vec::new();
    for e in entries {
        let template = template_map.get(e.template_name.as_str());

        let (processed_layout, derived_status, availability_status) =
            if let Some(template) = template.copied() {
                let layout = logs_db::process_template_layout_with_period_string(
                    &template.template_layout,
                    &e.period,
                );
                let (status, availability) = logs_db::derive_log_status(
                    e.status,
                    &template.schedule,
                    &e.period,
                    chrono::Utc::now(),
                );
                (layout, status, availability)
            } else {
                (
                    Vec::new(),
                    e.status,
                    logs_db::AvailabilityStatus::NotAvailable,
                )
            };

        response_entries.push(LogEntryResponse {
            id: e.entry_id,
            template_name: e.template_name,
            template_layout: processed_layout,
            entry_data: e.entry_data,
            status: derived_status.as_str().to_string(),
            availability_status: availability_status.as_str().to_string(),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
            submitted_at: e.submitted_at.map(|ts| ts.to_rfc3339()),
            period: e.period,
        });

        if let Some(status) = params.get("status")
            && let Some(filter_status) = logs_db::LogStatus::from_str(status)
            && derived_status != filter_status
        {
            response_entries.pop();
        }
    }

    Ok(Json(ListLogEntriesResponse {
        entries: response_entries,
    }))
}

#[utoipa::path(
    post,
    path = "/reports/runs",
    request_body = CreateReportRunRequest,
    responses(
        (status = 201, description = "Saved report run created", body = CreateReportRunResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Saves report-generation parameters for later reuse.
pub async fn create_report_run(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
    Json(mut payload): Json<CreateReportRunRequest>,
) -> Result<(StatusCode, Json<CreateReportRunResponse>), crate::error::AppError> {
    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    if payload.params.date_from_iso.is_empty() || payload.params.date_to_iso.is_empty() {
        return Err(crate::error::AppError::BadRequest("date_from_iso and date_to_iso are required".to_string()));
    }

    payload.params = logs_db::normalize_report_params(&payload.params);

    let now = chrono::Utc::now();
    let report_id = Uuid::new_v4().to_string();
    let doc = logs_db::ReportRunDocument {
        report_id: report_id.clone(),
        user_id: user.id,
        company_id,
        name: payload.name,
        params: payload.params,
        params_key: String::new(),
        created_at: now,
        last_used_at: now,
        use_count: 1,
    };

    let saved = logs_db::create_report_run(&state.mongodb, &doc)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to save report run".to_string())
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateReportRunResponse {
            report_run: ReportRunResponse {
                id: saved.report_id,
                name: saved.name,
                params: saved.params,
                created_at: saved.created_at.to_rfc3339(),
                last_used_at: saved.last_used_at.to_rfc3339(),
                use_count: saved.use_count,
            },
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/reports/runs",
    params(("limit" = u32, Query, description = "Optional limit for report run list, default 20, max 100")),
    responses(
        (status = 200, description = "Saved report runs retrieved", body = ListReportRunsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Lists saved report-generation parameter sets for the current user.
pub async fn list_report_runs(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ListReportRunsResponse>, crate::error::AppError> {
    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);

    let runs = logs_db::list_report_runs(&state.mongodb, &user.id, &company_id, limit)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to list report runs".to_string())
        })?;

    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for run in runs {
        let key = if run.params_key.is_empty() {
            logs_db::report_params_key(&run.params).map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to list report runs".to_string())
        })?
        } else {
            run.params_key.clone()
        };

        if seen.insert(key) {
            deduped.push(run);
        }
    }

    Ok(Json(ListReportRunsResponse {
        report_runs: deduped
            .into_iter()
            .map(|run| ReportRunResponse {
                id: run.report_id,
                name: run.name,
                params: run.params,
                created_at: run.created_at.to_rfc3339(),
                last_used_at: run.last_used_at.to_rfc3339(),
                use_count: run.use_count,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/reports/runs/{report_id}/use",
    params(("report_id" = String, Path, description = "Saved report run ID")),
    responses(
        (status = 200, description = "Saved report run usage tracked", body = UseReportRunResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Report run not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Marks a saved report run as used.
pub async fn use_report_run(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
    Path(report_id): Path<String>,
) -> Result<Json<UseReportRunResponse>, crate::error::AppError> {
    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    let touched = logs_db::touch_report_run(&state.mongodb, &report_id, &user.id, &company_id)
        .await
        .map_err(|e| {
            tracing::error!("Error: {:?}", e);
            crate::error::AppError::Internal("Failed to update report run usage".to_string())
        })?;

    if !touched {
        return Err(crate::error::AppError::NotFound("Report run not found".to_string()));
    }

    Ok(Json(UseReportRunResponse {
        message: "Report run marked as used".to_string(),
    }))
}

#[utoipa::path(
    delete,
    path = "/reports/runs/{report_id}",
    params(("report_id" = String, Path, description = "Saved report run ID")),
    responses(
        (status = 200, description = "Saved report run deleted", body = DeleteReportRunResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Report run not found", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Log Entries"
)]
/// Deletes a saved report run.
pub async fn delete_report_run(
    ReadBranchUser(_claims, user): ReadBranchUser,
    State(state): State<AppState>,
    Path(report_id): Path<String>,
) -> Result<Json<DeleteReportRunResponse>, crate::error::AppError> {
    let company_id = user.company_id.ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;

    tracing::info!(
        target: "report_runs",
        "delete requested: report_id={}, user_id={}, company_id={}",
        report_id,
        user.id,
        company_id
    );

    let deleted = logs_db::delete_report_run(&state.mongodb, &report_id, &user.id, &company_id)
        .await
        .map_err(|e| {
            tracing::error!(
                target: "report_runs",
                "delete failed: report_id={}, user_id={}, company_id={}, err={:?}",
                report_id,
                user.id,
                company_id,
                e
            );
            crate::error::AppError::Internal("Failed to delete report run" .to_string())
        })?;

    if !deleted {
        tracing::warn!(
            target: "report_runs",
            "delete not found: report_id={}, user_id={}, company_id={}",
            report_id,
            user.id,
            company_id
        );
        return Err(crate::error::AppError::NotFound("Report run not found".to_string()));
    }

    tracing::info!(
        target: "report_runs",
        "delete succeeded: report_id={}, user_id={}, company_id={}",
        report_id,
        user.id,
        company_id
    );

    Ok(Json(DeleteReportRunResponse {
        message: "Report run deleted".to_string(),
    }))
}
