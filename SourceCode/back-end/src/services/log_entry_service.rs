use crate::{
    AppState,
    db::{self, UserRecord},
    error::AppError,
    logs_db,
};
use uuid::Uuid;

#[cfg(test)]
mod log_entry_service_tests {
    use super::*;
    use chrono::Utc;

    fn create_test_entry(user_id: &str, company_id: &str) -> logs_db::LogEntry {
        logs_db::LogEntry {
            entry_id: Uuid::new_v4().to_string(),
            template_name: "test_template".to_string(),
            company_id: company_id.to_string(),
            branch_id: Some("branch_123".to_string()),
            user_id: user_id.to_string(),
            entry_data: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            submitted_at: None,
            status: logs_db::LogStatus::Draft,
            period: "2024-05".to_string(),
        }
    }

    #[test]
    fn test_log_entry_creation_initializes_correctly() {
        let user_id = "user_123";
        let company_id = "company_456";
        let entry = create_test_entry(user_id, company_id);

        assert_eq!(entry.user_id, user_id);
        assert_eq!(entry.company_id, company_id);
        assert_eq!(entry.status, logs_db::LogStatus::Draft);
        assert!(entry.submitted_at.is_none());
        assert_eq!(entry.template_name, "test_template");
    }

    #[test]
    fn test_log_entry_different_users_creates_unique_entries() {
        let user1 = "user_1";
        let user2 = "user_2";
        let company_id = "company_789";

        let entry1 = create_test_entry(user1, company_id);
        let entry2 = create_test_entry(user2, company_id);

        assert_ne!(entry1.entry_id, entry2.entry_id);
        assert_ne!(entry1.user_id, entry2.user_id);
        assert_eq!(entry1.company_id, entry2.company_id);
    }

    #[test]
    fn test_log_entry_status_is_draft_on_creation() {
        let user_id = "user_123";
        let company_id = "company_456";
        let entry = create_test_entry(user_id, company_id);

        assert_eq!(entry.status, logs_db::LogStatus::Draft);
    }

    #[test]
    fn test_log_entry_period_field_populated() {
        let entry = create_test_entry("user_123", "company_456");

        assert!(!entry.period.is_empty());
        assert_eq!(entry.period, "2024-05");
    }

    #[test]
    fn test_log_entry_unique_ids() {
        let entry1 = create_test_entry("user_123", "company_456");
        let entry2 = create_test_entry("user_123", "company_456");

        assert_ne!(entry1.entry_id, entry2.entry_id);
    }
}

pub struct LogEntryService;

impl LogEntryService {
    /// Creates a new log entry draft based on a template.
    ///
    /// # Errors
    /// Returns an error if the user has no company, the template is not found, or an entry already exists for the period.
    pub async fn create_log_entry(
        state: &AppState,
        user: &UserRecord,
        template_name: &str,
        period: Option<&str>,
    ) -> Result<String, AppError> {
        let company_id = user.company_id.as_ref().ok_or(AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;

        let template = logs_db::get_template_by_name(&state.mongodb, template_name, company_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get template: {:?}", e);
                AppError::Internal("Failed to get template".to_string())
            })?
            .ok_or(AppError::NotFound("Template not found".to_string()))?;

        match user.role {
            db::UserRole::BranchManager => {
                if Some(template.branch_id.as_ref()) != Some(user.branch_id.as_ref()) {
                    return Err(AppError::Forbidden(
                        "Template is not available for your branch".to_string(),
                    ));
                }
            }
            db::UserRole::Staff => {
                if !user.is_readonly_hq()
                    && Some(template.branch_id.as_ref()) != Some(user.branch_id.as_ref())
                {
                    return Err(AppError::Forbidden(
                        "Template is not available for your branch".to_string(),
                    ));
                }
            }
            db::UserRole::CompanyManager | db::UserRole::LogSmartAdmin => {
                // Company admins and LogSmart admins can access templates for any branch in their company, so no branch check needed here
            }
        }

        let now = chrono::Utc::now();
        let period_to_use = match period {
            Some(p) => {
                match logs_db::validate_period_business_rules(
                    &template.schedule,
                    p,
                    Some(template.created_at),
                    now,
                ) {
                    Ok(normalized) => normalized,
                    Err(err) => {
                        return Err(AppError::BadRequest(match err {
                            logs_db::PeriodValidationError::FormatInvalid => format!(
                                "Invalid period format for {} schedule",
                                match template.schedule.frequency {
                                    logs_db::Frequency::Daily => "daily",
                                    logs_db::Frequency::Weekly => "weekly",
                                    logs_db::Frequency::Monthly => "monthly",
                                    logs_db::Frequency::Quarterly => "quarterly",
                                    logs_db::Frequency::Yearly => "yearly",
                                }
                            ),
                            logs_db::PeriodValidationError::DueDateInFuture => {
                                "Period due date cannot be in the future".to_string()
                            }
                            logs_db::PeriodValidationError::WeekdayNotAllowed => {
                                "Period weekday is not allowed for this schedule".to_string()
                            }
                            logs_db::PeriodValidationError::BeforeTemplateCreation => {
                                "Period cannot be before template creation date".to_string()
                            }
                        }));
                    }
                }
            }
            None => logs_db::format_period_for_frequency(&template.schedule.frequency),
        };

        let has_entry = logs_db::has_entry_for_period(
            &state.mongodb,
            company_id,
            template_name,
            &period_to_use,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to check for existing entries: {:?}", e);
            AppError::Internal("Failed to check for existing entries".to_string())
        })?;

        if has_entry {
            return Err(AppError::Conflict(format!(
                "A log entry for this template has already been created for period {period_to_use}"
            )));
        }

        let entry_id = Uuid::new_v4().to_string();
        let period = period_to_use;

        let entry = logs_db::LogEntry {
            entry_id: entry_id.clone(),
            template_name: template_name.to_string(),
            company_id: company_id.clone(),
            branch_id: template.branch_id.clone(),
            user_id: user.id.clone(),
            entry_data: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            submitted_at: None,
            status: logs_db::LogStatus::Draft,
            period,
        };

        logs_db::create_log_entry(&state.mongodb, &entry)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create log entry: {:?}", e);
                AppError::Internal("Failed to create log entry".to_string())
            })?;

        Ok(entry_id)
    }

    /// Retrieves a specific log entry.
    ///
    /// # Errors
    /// Returns an error if the entry is not found or if the user doesn't have permission to view it.
    pub async fn get_log_entry(
        state: &AppState,
        user: &UserRecord,
        entry_id: &str,
    ) -> Result<logs_db::LogEntry, AppError> {
        let entry = logs_db::get_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get log entry: {:?}", e);
                AppError::Internal("Failed to get log entry".to_string())
            })?
            .ok_or(AppError::NotFound("Entry not found".to_string()))?;

        // Check if user owns the entry or has management permissions (including readonly HQ)
        if entry.user_id != user.id {
            // Allow if user can manage branch or is readonly HQ
            if !user.can_read_manage_branch() {
                return Err(AppError::Forbidden(
                    "You do not have permission to view this entry".to_string(),
                ));
            }

            // Additional check: ensure entry belongs to same company
            let user_company_id = user.company_id.as_ref().ok_or(AppError::Forbidden(
                "User is not associated with a company".to_string(),
            ))?;

            if &entry.company_id != user_company_id {
                return Err(AppError::Forbidden(
                    "You do not have permission to view this entry".to_string(),
                ));
            }
        }

        Ok(entry)
    }

    /// Updates an existing log entry draft.
    ///
    /// # Errors
    /// Returns an error if the entry is not found, user doesn't own it, or update fails.
    pub async fn update_log_entry(
        state: &AppState,
        user_id: &str,
        entry_id: &str,
        entry_data: &serde_json::Value,
    ) -> Result<logs_db::LogEntry, AppError> {
        let entry = logs_db::get_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get log entry: {:?}", e);
                AppError::Internal("Failed to get log entry".to_string())
            })?
            .ok_or(AppError::NotFound("Entry not found".to_string()))?;

        if entry.user_id != user_id {
            return Err(AppError::Forbidden(
                "You do not have permission to update this entry".to_string(),
            ));
        }

        let updated_entry =
            logs_db::update_log_entry_with_return(&state.mongodb, entry_id, entry_data)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to update log entry: {:?}", e);
                    AppError::Internal("Failed to update log entry".to_string())
                })?
                .ok_or(AppError::NotFound("Entry not found".to_string()))?;

        Ok(updated_entry)
    }

    /// Submits a log entry, marking it as final.
    ///
    /// # Errors
    /// Returns an error if the entry is not found, user doesn't own it, or submission fails.
    pub async fn submit_log_entry(
        state: &AppState,
        user_id: &str,
        entry_id: &str,
    ) -> Result<(), AppError> {
        let entry = logs_db::get_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get log entry: {:?}", e);
                AppError::Internal("Failed to get log entry".to_string())
            })?
            .ok_or(AppError::NotFound("Entry not found".to_string()))?;

        if entry.user_id != user_id {
            return Err(AppError::Forbidden(
                "You do not have permission to submit this entry".to_string(),
            ));
        }

        logs_db::submit_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to submit log entry: {:?}", e);
                AppError::Internal("Failed to submit log entry".to_string())
            })?;

        Ok(())
    }

    /// Returns a submitted log entry to draft status (admin only).
    ///
    /// # Errors
    /// Returns an error if the user is not an admin, entry is not found, or operation fails.
    pub async fn unsubmit_log_entry(
        state: &AppState,
        user: &UserRecord,
        entry_id: &str,
    ) -> Result<(), AppError> {
        let entry = logs_db::get_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get log entry: {:?}", e);
                AppError::Internal("Failed to get log entry".to_string())
            })?
            .ok_or(AppError::NotFound("Entry not found".to_string()))?;

        let user_company_id = db::get_user_company_id(&state.postgres, &user.id)
            .await
            .map_err(|e| {
                tracing::error!("Database error fetching user company ID: {:?}", e);
                AppError::Internal("Database error".to_string())
            })?;

        if let Some(company_id) = user_company_id {
            if entry.company_id != company_id && !user.is_logsmart_admin() {
                return Err(AppError::Forbidden(
                    "You do not have permission to unsubmit this entry".to_string(),
                ));
            }
        } else if !user.is_logsmart_admin() {
            return Err(AppError::Forbidden(
                "User is not associated with a company".to_string(),
            ));
        }

        logs_db::unsubmit_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to unsubmit log entry: {:?}", e);
                AppError::Internal("Failed to unsubmit log entry".to_string())
            })?;

        Ok(())
    }

    /// Deletes a log entry.
    ///
    /// # Errors
    /// Returns an error if the entry is not found, user is not authorized, or deletion fails.
    pub async fn delete_log_entry(
        state: &AppState,
        user: &UserRecord,
        entry_id: &str,
    ) -> Result<(), AppError> {
        let entry = logs_db::get_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get log entry: {:?}", e);
                AppError::Internal("Failed to get log entry".to_string())
            })?
            .ok_or(AppError::NotFound("Entry not found".to_string()))?;

        if user.is_staff() && entry.user_id != user.id {
            return Err(AppError::Forbidden(
                "You may not delete log entries created by other users".to_string(),
            ));
        }

        if user.is_branch_manager() && entry.branch_id != user.branch_id {
            return Err(AppError::Forbidden(
                "You do not have permission to delete entries from another branch".to_string(),
            ));
        }

        if user.is_company_manager() && Some(entry.company_id) != user.company_id {
            return Err(AppError::Forbidden(
                "You do not have permission to delete entries from another company".to_string(),
            ));
        }

        logs_db::delete_log_entry(&state.mongodb, entry_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to delete log entry: {:?}", e);
                AppError::Internal("Failed to delete log entry".to_string())
            })?;

        Ok(())
    }

    /// Lists all log templates for a company.
    ///
    /// # Errors
    /// Returns an error if the database query fails.
    pub async fn list_due_forms(
        state: &AppState,
        company_id: &str,
        branch_id: Option<&str>,
    ) -> Result<Vec<logs_db::TemplateDocument>, AppError> {
        logs_db::get_templates_by_company_and_branch(&state.mongodb, company_id, branch_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get templates: {:?}", e);
                AppError::Internal("Failed to get templates".to_string())
            })
    }

    pub async fn get_user_log_entries(
        state: &AppState,
        user_id: &str,
        company_id: &str,
    ) -> Result<Vec<logs_db::LogEntry>, AppError> {
        logs_db::get_user_log_entries(&state.mongodb, user_id, company_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get log entries: {:?}", e);
                AppError::Internal("Failed to get log entries".to_string())
            })
    }
}
