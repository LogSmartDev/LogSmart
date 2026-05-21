use anyhow::Result;
use sqlx::PgPool;

mod types;
mod user_repo;
mod company_repo;
mod branch_repo;
mod auth_repo;
mod security_repo;
mod clock_repo;
mod health_repo;

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

const CLOCK_EVENT_RETURNING: &str = "RETURNING id, user_id, company_id, clock_in, clock_out, created_at";

const BRANCH_SELECT_COLUMNS: &str = "SELECT id, company_id, name, address, created_at \
       FROM branches";

const COMPANY_RETURNING_COLUMNS: &str = "RETURNING id, name, address, created_at, logo_id, data_exported_at, deleted_at, deletion_requested_at, deletion_token, deletion_requested_by_email";

pub fn encode_cursor(created_at: chrono::DateTime<chrono::Utc>, id: &str) -> String {
    let json = serde_json::json!({
        "created_at": created_at.to_rfc3339(),
        "id": id
    });
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(json.to_string().as_bytes())
}

pub fn decode_cursor(cursor: &str) -> Result<(chrono::DateTime<chrono::Utc>, String)> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let bytes = URL_SAFE_NO_PAD.decode(cursor)?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)?;
    let created_at = chrono::DateTime::parse_from_rfc3339(json["created_at"].as_str().ok_or_else(|| anyhow::anyhow!("missing created_at"))?)?;
    let id = json["id"].as_str().ok_or_else(|| anyhow::anyhow!("missing id"))?.to_string();
    Ok((created_at.with_timezone(&chrono::Utc), id))
}

/// Initialize database by running `SQLx` migrations
///
/// This function runs all pending migrations from the `migrations/` directory.
/// Migrations are applied in order based on their timestamp prefix.
///
/// # Errors
/// Returns an error if migrations fail to execute.
pub async fn init_db(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run database migrations: {e}"))?;

    Ok(())
}

// Re-export all types
pub use types::*;

// Re-export all user repo functions
pub use user_repo::{
    create_user,
    get_user_company_id,
    get_user_by_email,
    get_user_by_id,
    get_user_by_oauth,
    create_oauth_user,
    link_oauth_to_user,
    unlink_oauth_from_user,
    delete_user_by_email,
    soft_delete_users_by_company_id,
    get_users_by_company_id,
    get_all_users_by_company_id,
    get_company_members_for_user,
    update_user_profile,
    update_user_profile_picture_id,
    update_user_profile_full,
    update_user_password,
    update_user_branch,
};

// Re-export all company repo functions
pub use company_repo::{
    create_company,
    update_company_logo_id,
    get_company_by_id,
    update_company,
    mark_company_data_exported,
    request_company_deletion,
    confirm_company_deletion,
};

// Re-export all branch repo functions
pub use branch_repo::{
    create_branch,
    get_branches_by_company_id_with_deletion_status,
    get_branch_by_id,
    update_branch,
    delete_branch,
    create_branch_deletion_token,
    get_branch_deletion_token,
    mark_branch_deletion_token_used,
};

// Re-export all auth repo functions
pub use auth_repo::{
    create_invitation,
    get_invitation_by_token,
    accept_invitation,
    cancel_invitation,
    get_invitation_by_id,
    accept_invitation_with_user_creation,
    create_passkey,
    get_passkeys_by_user,
    get_passkey_by_credential_id,
    update_passkey_usage,
    delete_passkey,
    create_password_reset_token,
    get_password_reset_by_token,
    mark_password_reset_used,
    create_passkey_session,
    get_passkey_session,
    delete_passkey_session,
    get_pending_invitations_by_company_id,
};

// Re-export all security repo functions
pub use security_repo::{
    log_security_event,
    get_security_logs_by_user,
    get_recent_security_logs,
    get_security_logs_page,
    get_security_logs_for_export,
    format_security_logs_cursor,
    parse_security_logs_cursor,
};

// Re-export all clock repo functions
pub use clock_repo::{
    clock_in,
    clock_out,
    clock_out_at,
    get_clock_status,
    get_recent_clock_events,
    get_company_clock_events,
    create_clock_events_cursor,
    parse_clock_events_cursor,
};

// Re-export all health repo functions
pub use health_repo::{
    get_database_health,
    get_slow_queries,
    get_index_usage,
    get_table_sizes,
    check_unused_indexes,
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
