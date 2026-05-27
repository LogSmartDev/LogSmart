use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type, ToSchema)]
#[sqlx(type_name = "user_role")]
#[sqlx(rename_all = "lowercase")]
pub enum UserRole {
    #[serde(rename = "logsmart_admin")]
    #[sqlx(rename = "logsmart_admin")]
    LogSmartAdmin,
    #[serde(rename = "company_manager")]
    #[sqlx(rename = "company_manager")]
    CompanyManager,
    #[serde(rename = "branch_manager")]
    #[sqlx(rename = "branch_manager")]
    BranchManager,
    #[serde(rename = "staff")]
    #[sqlx(rename = "staff")]
    Staff,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::LogSmartAdmin => write!(f, "logsmart_admin"),
            UserRole::CompanyManager => write!(f, "company_manager"),
            UserRole::BranchManager => write!(f, "branch_manager"),
            UserRole::Staff => write!(f, "staff"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "logsmart_admin" => Ok(UserRole::LogSmartAdmin),
            "company_manager" => Ok(UserRole::CompanyManager),
            "branch_manager" => Ok(UserRole::BranchManager),
            "staff" => Ok(UserRole::Staff),
            _ => Err(format!("Unknown role: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDisplay {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub company_name: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub password_hash: Option<String>,
    pub company_id: Option<String>,
    pub branch_id: Option<String>,
    pub company_name: Option<String>,
    pub company_deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub role: UserRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub oauth_provider: Option<String>,
    pub oauth_subject: Option<String>,
    pub oauth_picture: Option<String>,
    pub profile_picture_id: Option<String>,
}

impl UserRecord {
    #[must_use]
    pub fn get_role(&self) -> UserRole {
        self.role.clone()
    }

    #[must_use]
    pub fn is_logsmart_admin(&self) -> bool {
        self.get_role() == UserRole::LogSmartAdmin
    }

    #[must_use]
    pub fn is_company_manager(&self) -> bool {
        self.get_role() == UserRole::CompanyManager
    }

    #[must_use]
    pub fn is_branch_manager(&self) -> bool {
        self.get_role() == UserRole::BranchManager
    }

    #[must_use]
    pub fn is_staff(&self) -> bool {
        self.get_role() == UserRole::Staff
    }

    #[must_use]
    pub fn can_manage_company(&self) -> bool {
        self.is_company_manager() || self.is_logsmart_admin()
    }

    #[must_use]
    pub fn can_manage_branch(&self) -> bool {
        self.is_branch_manager() || self.can_manage_company()
    }

    #[must_use]
    pub fn is_readonly_hq(&self) -> bool {
        self.is_staff() && self.branch_id.is_none()
    }

    #[must_use]
    pub fn can_read_manage_branch(&self) -> bool {
        self.is_readonly_hq() || self.can_manage_branch()
    }

    /// Returns the company_id as an owned String if present, or a Forbidden error if not.
    pub fn company_id_or_forbidden(&self) -> Result<String, crate::error::AppError> {
        self.company_id.clone().ok_or_else(|| {
            crate::error::AppError::Forbidden("User is not associated with a company".to_string())
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct Branch {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Company {
    pub id: String,
    pub name: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub logo_id: Option<String>,
    pub data_exported_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deletion_requested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deletion_token: Option<String>,
    pub deletion_requested_by_email: Option<String>,
}

impl Default for Company {
    fn default() -> Self {
        Self::new()
    }
}

impl Company {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            address: String::new(),
            created_at: chrono::Utc::now(),
            logo_id: None,
            data_exported_at: None,
            deleted_at: None,
            deletion_requested_at: None,
            deletion_token: None,
            deletion_requested_by_email: None,
        }
    }

    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    #[must_use]
    pub fn with_name_and_address(mut self, name: &str, address: &str) -> Self {
        self.name = name.to_string();
        self.address = address.to_string();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Invitation {
    pub id: String,
    pub company_id: String,
    pub email: String,
    pub token: String,
    pub role: UserRole,
    pub branch_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub accepted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SecurityLog {
    pub id: String,
    pub event_type: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub actor_role: Option<String>,
    pub company_id: Option<String>,
    pub target_user_id: Option<String>,
    pub target_email: Option<String>,
    pub request_path: Option<String>,
    pub request_method: Option<String>,
    pub details: Option<String>,
    pub success: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct SecurityLogMeta {
    pub actor_role: Option<String>,
    pub company_id: Option<String>,
    pub target_user_id: Option<String>,
    pub target_email: Option<String>,
    pub request_path: Option<String>,
    pub request_method: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecurityLogFilters {
    pub event_type: Option<String>,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub actor_role: Option<String>,
    pub company_id: Option<String>,
    pub target_user_id: Option<String>,
    pub target_email: Option<String>,
    pub request_path: Option<String>,
    pub request_method: Option<String>,
    pub details: Option<String>,
    pub success: Option<bool>,
    pub created_from: Option<chrono::DateTime<chrono::Utc>>,
    pub created_to: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct SecurityLogsPage {
    pub logs: Vec<SecurityLog>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Passkey {
    pub id: String,
    pub user_id: String,
    pub credential_id: String,
    pub public_key: String,
    pub counter: i64,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PasskeySession {
    pub id: String,
    pub session_type: String,
    pub user_id: Option<String>,
    pub challenge: String,
    pub meta: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ClockEvent {
    pub id: String,
    pub user_id: String,
    pub company_id: String,
    pub clock_in: chrono::DateTime<chrono::Utc>,
    pub clock_out: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ClockEvent {
    #[must_use]
    pub fn is_clocked_in(&self) -> bool {
        self.clock_out.is_none()
    }
}

/// Branch with deletion status information
pub struct BranchWithDeletionStatus {
    pub branch: Branch,
    pub has_pending_deletion: bool,
    pub deletion_requested_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A clock event row joined with user info, for company-wide reporting.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyClockEventRow {
    pub id: String,
    pub user_id: String,
    pub company_id: String,
    pub clock_in: chrono::DateTime<chrono::Utc>,
    pub clock_out: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

impl CompanyClockEventRow {
    #[must_use]
    pub fn is_clocked_in(&self) -> bool {
        self.clock_out.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DatabaseHealthMetrics {
    pub total_connections: i64,
    pub active_connections: i64,
    pub idle_connections: i64,
    pub max_connections: i32,
    pub database_size_mb: f64,
    pub table_count: i64,
    pub index_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct SlowQueryInfo {
    pub query: String,
    pub calls: i64,
    pub total_time_ms: f64,
    pub mean_time_ms: f64,
    pub max_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct IndexUsageStats {
    pub table_name: String,
    pub index_name: String,
    pub index_scans: i64,
    pub rows_read: i64,
    pub rows_fetched: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TableSizeInfo {
    pub table_name: String,
    pub row_count: i64,
    pub total_size_mb: f64,
    pub table_size_mb: f64,
    pub index_size_mb: f64,
}
