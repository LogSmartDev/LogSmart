use crate::error::DbError;
use sqlx::PgPool;

mod auth_repo;
mod branch_repo;
mod clock_repo;
mod company_repo;
mod health_repo;
mod security_repo;
mod types;
mod user_repo;

// Reusable SELECT column constants to reduce boilerplate
const USER_SELECT_COLUMNS: &str = "SELECT users.id, users.email, users.first_name, users.last_name, \
       users.password_hash, users.company_id, users.branch_id, users.role, users.created_at, users.deleted_at, \
       companies.name as company_name, companies.deleted_at as company_deleted_at, \
       users.oauth_provider, users.oauth_subject, users.oauth_picture, users.profile_picture_id \
       FROM users \
       LEFT JOIN companies ON users.company_id = companies.id";

const SECURITY_LOG_SELECT_COLUMNS: &str = "SELECT id, event_type, user_id, email, ip_address, user_agent, actor_role, company_id, target_user_id, target_email, request_path, request_method, details, success, created_at \
       FROM security_logs";

const CLOCK_EVENT_SELECT_COLUMNS: &str = "SELECT id, user_id, company_id, clock_in, clock_out, created_at \
       FROM clock_events";

const CLOCK_EVENT_RETURNING: &str =
    "RETURNING id, user_id, company_id, clock_in, clock_out, created_at";

const BRANCH_SELECT_COLUMNS: &str = "SELECT id, company_id, name, address, created_at \
       FROM branches";

const COMPANY_RETURNING_COLUMNS: &str = "RETURNING id, name, address, created_at, logo_id, data_exported_at, deleted_at, deletion_requested_at, deletion_token, deletion_requested_by_email";

pub fn encode_cursor(created_at: chrono::DateTime<chrono::Utc>, id: &str) -> String {
    let json = serde_json::json!({
        "created_at": created_at.to_rfc3339(),
        "id": id
    });
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.encode(json.to_string().as_bytes())
}

pub fn decode_cursor(cursor: &str) -> Result<(chrono::DateTime<chrono::Utc>, String), DbError> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|e| DbError::Internal(format!("Failed to decode cursor: {e}")))?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| DbError::Internal(format!("Failed to parse cursor JSON: {e}")))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(
        json["created_at"]
            .as_str()
            .ok_or_else(|| DbError::Internal("missing created_at".to_string()))?,
    )
    .map_err(|e| DbError::Internal(format!("Failed to parse created_at: {e}")))?;
    let id = json["id"]
        .as_str()
        .ok_or_else(|| DbError::Internal("missing id".to_string()))?
        .to_string();
    Ok((created_at.with_timezone(&chrono::Utc), id))
}

/// Initialize database by running `SQLx` migrations
///
/// This function runs all pending migrations from the `migrations/` directory.
/// Migrations are applied in order based on their timestamp prefix.
///
/// # Errors
/// Returns an error if migrations fail to execute.
pub async fn init_db(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| DbError::Internal(format!("Failed to run database migrations: {e}")))?;

    Ok(())
}

// Re-export all types
pub use types::*;

// Re-export all user repo functions
pub use user_repo::{
    create_oauth_user, create_user, delete_user_by_email, get_all_users_by_company_id,
    get_company_members_for_user, get_user_by_email, get_user_by_id, get_user_by_oauth,
    get_user_company_id, get_users_by_company_id, link_oauth_to_user,
    soft_delete_users_by_company_id, unlink_oauth_from_user, update_user_branch,
    update_user_password, update_user_profile, update_user_profile_full,
    update_user_profile_picture_id,
};

// Re-export all company repo functions
pub use company_repo::{
    confirm_company_deletion, create_company, get_company_by_id, mark_company_data_exported,
    request_company_deletion, update_company, update_company_logo_id,
};

// Re-export all branch repo functions
pub use branch_repo::{
    create_branch, create_branch_deletion_token, delete_branch, get_branch_by_id,
    get_branch_deletion_token, get_branches_by_company_id_with_deletion_status,
    mark_branch_deletion_token_used, update_branch,
};

// Re-export all auth repo functions
pub use auth_repo::{
    accept_invitation, accept_invitation_with_user_creation, cancel_invitation, create_invitation,
    create_passkey, create_passkey_session, create_password_reset_token, delete_passkey,
    delete_passkey_session, get_invitation_by_id, get_invitation_by_token,
    get_passkey_by_credential_id, get_passkey_session, get_passkeys_by_user,
    get_password_reset_by_token, get_pending_invitations_by_company_id, mark_password_reset_used,
    update_passkey_usage,
};

// Re-export all security repo functions
pub use security_repo::{
    format_security_logs_cursor, get_recent_security_logs, get_security_logs_by_user,
    get_security_logs_for_export, get_security_logs_page, log_security_event,
    parse_security_logs_cursor,
};

// Re-export all clock repo functions
pub use clock_repo::{
    clock_in, clock_out, clock_out_at, create_clock_events_cursor, get_clock_status,
    get_company_clock_events, get_recent_clock_events, parse_clock_events_cursor,
};

// Re-export all health repo functions
pub use health_repo::{
    check_unused_indexes, get_database_health, get_index_usage, get_slow_queries, get_table_sizes,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_clocked_in_with_future_clock_in() {
        let future_time = chrono::Utc::now() + chrono::Duration::seconds(60);
        let event = ClockEvent {
            id: "test".to_string(),
            user_id: "user1".to_string(),
            company_id: "company1".to_string(),
            clock_in: future_time,
            clock_out: None,
            created_at: chrono::Utc::now(),
        };
        assert!(
            event.is_clocked_in(),
            "ClockEvent with clock_out=None should be clocked in even if clock_in is in the future"
        );
    }

    #[test]
    fn test_is_clocked_in_with_past_clock_in() {
        let past_time = chrono::Utc::now() - chrono::Duration::seconds(60);
        let event = ClockEvent {
            id: "test".to_string(),
            user_id: "user1".to_string(),
            company_id: "company1".to_string(),
            clock_in: past_time,
            clock_out: None,
            created_at: chrono::Utc::now(),
        };
        assert!(
            event.is_clocked_in(),
            "ClockEvent with clock_out=None should be clocked in"
        );
    }

    #[test]
    fn test_is_clocked_in_with_clock_out() {
        let past_time = chrono::Utc::now() - chrono::Duration::seconds(60);
        let event = ClockEvent {
            id: "test".to_string(),
            user_id: "user1".to_string(),
            company_id: "company1".to_string(),
            clock_in: past_time,
            clock_out: Some(past_time + chrono::Duration::seconds(60)),
            created_at: chrono::Utc::now(),
        };
        assert!(
            !event.is_clocked_in(),
            "ClockEvent with clock_out set should not be clocked in"
        );
    }

    #[test]
    fn test_company_clock_event_row_is_clocked_in() {
        let future_time = chrono::Utc::now() + chrono::Duration::seconds(60);
        let row = CompanyClockEventRow {
            id: "test".to_string(),
            user_id: "user1".to_string(),
            company_id: "company1".to_string(),
            clock_in: future_time,
            clock_out: None,
            created_at: chrono::Utc::now(),
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            email: "john@example.com".to_string(),
        };
        assert!(
            row.is_clocked_in(),
            "CompanyClockEventRow with clock_out=None should be clocked in even if clock_in is in the future"
        );
    }
}
