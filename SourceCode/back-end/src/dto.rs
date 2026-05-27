use crate::{
    db::{self, UserRole},
    logs_db,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

#[macro_export]
macro_rules! derive_from {
    ($from:ty, $to:ident, [$($field:ident),* $(,)?]) => {
        impl From<$from> for $to {
            fn from(source: $from) -> Self {
                Self {
                    $($field: source.$field),*
                }
            }
        }
    };
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminUpdateMemberRequest {
    #[schema(example = "user@example.com")]
    pub email: String,
    #[schema(example = "Jane")]
    pub first_name: String,
    #[schema(example = "Smith")]
    pub last_name: String,
    #[schema(example = "staff")]
    pub role: String,
    #[schema(example = "branch-uuid-here")]
    pub branch_id: Option<String>,
    #[schema(example = "uuid-of-profile-picture")]
    pub profile_picture_id: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AdminProfilePictureQuery {
    #[schema(example = "user@example.com")]
    pub email: Option<String>,
}
#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelInvitationRequest {
    #[schema(example = "invitation-uuid-here")]
    pub invitation_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RemoveMemberRequest {
    #[schema(example = "user@example.com")]
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub company_id: Option<String>,
    pub branch_id: Option<String>,
    pub role: UserRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

derive_from!(
    crate::db::UserRecord,
    UserDto,
    [
        id, email, first_name, last_name, company_id, branch_id, role, created_at
    ]
);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BranchDto {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub has_pending_deletion: bool,
    #[serde(default)]
    pub deletion_requested_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<db::Branch> for BranchDto {
    fn from(branch: db::Branch) -> Self {
        Self {
            id: branch.id,
            company_id: branch.company_id,
            name: branch.name,
            address: branch.address,
            created_at: branch.created_at,
            has_pending_deletion: false,
            deletion_requested_at: None,
        }
    }
}

impl From<db::BranchWithDeletionStatus> for BranchDto {
    fn from(branch_with_status: db::BranchWithDeletionStatus) -> Self {
        Self {
            id: branch_with_status.branch.id,
            company_id: branch_with_status.branch.company_id,
            name: branch_with_status.branch.name,
            address: branch_with_status.branch.address,
            created_at: branch_with_status.branch.created_at,
            has_pending_deletion: branch_with_status.has_pending_deletion,
            deletion_requested_at: branch_with_status.deletion_requested_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateBranchRequest {
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "London Office")]
    pub name: String,
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "123 Regent St, London")]
    pub address: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateBranchRequest {
    #[validate(length(min = 1, max = 36), custom(function = "validate_uuid_format"))]
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub branch_id: String,
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "London Office")]
    pub name: String,
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "123 Regent St, London")]
    pub address: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RequestBranchDeletionRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub branch_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RequestBranchDeletionResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfirmBranchDeletionRequest {
    #[schema(example = "deletion-token-here")]
    pub token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConfirmBranchDeletionResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListBranchesResponse {
    pub branches: Vec<BranchDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyDto {
    pub id: String,
    pub name: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub logo_id: Option<String>,
    pub data_exported_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deletion_requested_at: Option<chrono::DateTime<chrono::Utc>>,
}

derive_from!(
    crate::db::Company,
    CompanyDto,
    [
        id,
        name,
        address,
        created_at,
        logo_id,
        data_exported_at,
        deleted_at,
        deletion_requested_at
    ]
);

derive_from!(
    crate::db::Invitation,
    InvitationDto,
    [
        id,
        company_id,
        email,
        token,
        created_at,
        expires_at,
        accepted_at
    ]
);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InvitationDto {
    pub id: String,
    pub company_id: String,
    pub email: String,
    pub token: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub accepted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetPendingInvitationsResponse {
    pub invitations: Vec<InvitationResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SecurityLogDto {
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

derive_from!(
    crate::db::SecurityLog,
    SecurityLogDto,
    [
        id,
        event_type,
        user_id,
        email,
        ip_address,
        user_agent,
        actor_role,
        company_id,
        target_user_id,
        target_email,
        request_path,
        request_method,
        details,
        success,
        created_at
    ]
);

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct SecurityLogsQuery {
    #[schema(example = 15)]
    pub limit: Option<i64>,
    #[schema(example = "MjAyNi0wNC0xM1QxMjozNDo1Nlo=fGFiYy0xMjM")]
    pub cursor: Option<String>,
    #[schema(example = "login")]
    pub event_type: Option<String>,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Option<String>,
    #[schema(example = "user@example.com")]
    pub email: Option<String>,
    #[schema(example = "203.0.113.42")]
    pub ip_address: Option<String>,
    #[schema(example = "Mozilla/5.0")]
    pub user_agent: Option<String>,
    #[schema(example = "company_manager")]
    pub actor_role: Option<String>,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub company_id: Option<String>,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub target_user_id: Option<String>,
    #[schema(example = "target@example.com")]
    pub target_email: Option<String>,
    #[schema(example = "/companies/123")]
    pub request_path: Option<String>,
    #[schema(example = "PUT")]
    pub request_method: Option<String>,
    #[schema(example = "Passkey verification failed")]
    pub details: Option<String>,
    #[schema(example = true)]
    pub success: Option<bool>,
    #[schema(example = "2026-04-01T00:00:00Z")]
    pub created_from: Option<String>,
    #[schema(example = "2026-04-13T23:59:59Z")]
    pub created_to: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SecurityLogsResponse {
    pub logs: Vec<SecurityLogDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate, ToSchema)]
pub struct RegisterRequest {
    #[validate(email)]
    #[schema(example = "admin@example.com")]
    pub email: String,
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "John")]
    pub first_name: String,
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "Doe")]
    pub last_name: String,
    #[validate(length(min = 8))]
    #[schema(example = "SecurePass123!")]
    pub password: String,
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Example Corp")]
    pub company_name: String,
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "123 Main St, City, Country")]
    pub company_address: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetCompanyMembersResponse {
    pub members: Vec<UserResponse>,
}

impl From<Vec<db::UserRecord>> for GetCompanyMembersResponse {
    fn from(members: Vec<db::UserRecord>) -> Self {
        Self {
            members: members.into_iter().map(UserResponse::from).collect(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct VerifyTokenRequest {
    #[schema(example = "jwt-token-here")]
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Validate, ToSchema)]
pub struct AddTemplateRequest {
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Kitchen Daily Log")]
    pub template_name: String,
    #[schema(example = "[\"field1\", \"field2\"]")]
    pub template_layout: logs_db::TemplateLayout,
    #[schema(example = "{\"frequency\": \"daily\", \"time\": \"08:00\"}")]
    pub schedule: logs_db::Schedule,
    #[schema(example = "branch-uuid-here")]
    pub branch_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateTemplateResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize, Validate, ToSchema)]
pub struct UpdateTemplateRequest {
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Kitchen Daily Log")]
    pub template_name: String,
    #[schema(example = "[\"field1\", \"field2\"]")]
    pub template_layout: Option<logs_db::TemplateLayout>,
    #[schema(example = "{\"frequency\": \"daily\", \"time\": \"08:00\"}")]
    pub schedule: Option<logs_db::Schedule>,
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Major Update")]
    pub version_name: Option<String>,
    #[schema(example = "branch-uuid-here")]
    pub branch_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TemplateVersionInfo {
    pub version: u16,
    pub version_name: Option<String>,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetTemplateVersionsResponse {
    pub versions: Vec<TemplateVersionInfo>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestoreTemplateVersionRequest {
    pub version: u16,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct GetTemplateRequest {
    #[schema(example = "Kitchen Daily Log")]
    pub template_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TemplateInfo {
    pub template_name: String,
    pub schedule: logs_db::Schedule,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetAllTemplatesResponse {
    pub templates: Vec<TemplateInfo>,
}

#[derive(Debug, Deserialize, Serialize, Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(email)]
    #[schema(example = "admin@example.com")]
    pub email: String,
    #[validate(length(min = 8, max = 128), custom(function = "validate_password_policy_attr"))]
    #[schema(example = "SecurePass123!")]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct InviteUserRequest {
    #[validate(email)]
    #[schema(example = "newmember@example.com")]
    pub email: String,
    #[schema(example = "staff")]
    pub role: Option<UserRole>,
    #[schema(example = "branch-uuid-here")]
    pub branch_id: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct AcceptInvitationRequest {
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "invitation-token-here")]
    pub token: String,
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "Alice")]
    pub first_name: String,
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "Smith")]
    pub last_name: String,
    #[validate(length(min = 8, max = 128), custom(function = "validate_password_policy_attr"))]
    #[schema(example = "MemberPass123!")]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct JwtVerifyResponse {
    pub email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub company_id: Option<String>,
    pub company_name: Option<String>,
    pub branch_id: Option<String>,
    pub role: UserRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub oauth_provider: Option<String>,
    pub profile_picture_url: Option<String>,
    pub oauth_picture: Option<String>,
}

impl From<db::UserRecord> for UserResponse {
    fn from(record: db::UserRecord) -> Self {
        let profile_picture_url = record
            .profile_picture_id
            .map(|id| format!("/api/auth/profile-picture/{id}"));

        Self {
            id: record.id,
            email: record.email,
            first_name: record.first_name,
            last_name: record.last_name,
            company_id: record.company_id,
            company_name: record.company_name,
            branch_id: record.branch_id,
            role: record.role,
            created_at: record.created_at,
            oauth_provider: record.oauth_provider,
            profile_picture_url,
            oauth_picture: record.oauth_picture,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct GetInvitationDetailsRequest {
    #[schema(example = "invitation-token-here")]
    pub token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetInvitationDetailsResponse {
    pub company_name: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvitationResponse {
    pub id: String,
    pub email: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProfileRequest {
    #[schema(example = "John")]
    pub first_name: String,
    #[schema(example = "Doe")]
    pub last_name: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RequestPasswordResetRequest {
    #[validate(email)]
    #[schema(example = "user@example.com")]
    pub email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AddTokenResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AddTemplateResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GetTemplateResponse {
    pub template_name: String,
    pub template_layout: logs_db::TemplateLayout,
    pub version: u16,
    pub version_name: Option<String>,
    pub branch_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PasswordResetResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "reset-token-here")]
    pub token: String,
    #[validate(length(min = 8, max = 128), custom(function = "validate_password_policy_attr"))]
    #[schema(example = "NewPassword123!")]
    pub new_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OAuthInitiateResponse {
    pub authorization_url: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OAuthCallbackRequest {
    #[schema(example = "authorization-code-from-google")]
    pub code: String,
    #[schema(example = "csrf-state-token")]
    pub state: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLogEntryRequest {
    #[schema(example = "Kitchen Daily Log")]
    pub template_name: String,
    #[schema(example = "17/03/2026")]
    pub period: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogEntryResponse {
    pub id: String,
    pub template_name: String,
    pub template_layout: logs_db::TemplateLayout,
    pub entry_data: serde_json::Value,
    pub status: String,
    pub availability_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub submitted_at: Option<String>,
    pub period: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLogEntryRequest {
    pub entry_data: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitLogEntryRequest {}

#[derive(Debug, Serialize, ToSchema)]
pub struct DueFormInfo {
    pub template_name: String,
    pub template_layout: logs_db::TemplateLayout,
    pub last_submitted: Option<String>,
    pub period: String,
    pub status: Option<String>,
    pub availability_status: String,
    pub available_from: Option<String>,
    pub due_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DueFormsResponse {
    pub forms: Vec<DueFormInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListLogEntriesResponse {
    pub entries: Vec<LogEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateLogEntryResponse {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubmitLogEntryResponse {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ReportRunParams {
    pub date_from_iso: String,
    pub date_to_iso: String,
    pub selected_branch_ids: Vec<String>,
    pub selected_log_type_ids: Vec<String>,
    pub arrange_by: String,
    pub include_temperature_graphs: bool,
    pub params_version: u16,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateReportRunRequest {
    pub name: Option<String>,
    pub params: ReportRunParams,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReportRunResponse {
    pub id: String,
    pub name: Option<String>,
    pub params: ReportRunParams,
    pub created_at: String,
    pub last_used_at: String,
    pub use_count: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateReportRunResponse {
    pub report_run: ReportRunResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListReportRunsResponse {
    pub report_runs: Vec<ReportRunResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UseReportRunResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteReportRunResponse {
    pub message: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RenameTemplateRequest {
    pub old_template_name: String,
    pub new_template_name: String,
}

#[derive(Serialize, ToSchema)]
pub struct RenameTemplateResponse {
    pub message: String,
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct DeleteTemplateRequest {
    pub template_name: String,
}

#[derive(Serialize, ToSchema)]
pub struct DeleteTemplateResponse {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct LayoutGenerationRequest {
    pub user_prompt: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct LayoutGenerationResponse {
    pub layout: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PasskeyRegistrationStartRequest {
    #[schema(example = "My MacBook Pro")]
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PasskeyRegistrationStartResponse {
    pub options: serde_json::Value,
    pub auth_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PasskeyRegistrationFinishRequest {
    pub credential: serde_json::Value,
    pub auth_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PasskeyAuthenticationStartRequest {
    #[schema(example = "user@example.com")]
    pub email: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PasskeyAuthenticationStartResponse {
    pub options: serde_json::Value,
    pub auth_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PasskeyAuthenticationFinishRequest {
    pub credential: serde_json::Value,
    pub auth_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListPasskeysResponse {
    pub passkeys: Vec<PasskeyDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PasskeyDto {
    pub id: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

derive_from!(
    db::Passkey,
    PasskeyDto,
    [id, name, created_at, last_used_at]
);

// Clock In/Out DTOs

#[derive(Debug, Serialize, ToSchema)]
pub struct ClockEventResponse {
    pub id: String,
    pub user_id: String,
    pub clock_in: chrono::DateTime<chrono::Utc>,
    pub clock_out: Option<chrono::DateTime<chrono::Utc>>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<db::ClockEvent> for ClockEventResponse {
    fn from(event: db::ClockEvent) -> Self {
        let status = if event.is_clocked_in() { "in" } else { "out" }.to_string();

        Self {
            id: event.id,
            user_id: event.user_id,
            clock_in: event.clock_in,
            clock_out: event.clock_out,
            status,
            created_at: event.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClockStatusResponse {
    pub is_clocked_in: bool,
    pub current_event: Option<ClockEventResponse>,
    pub recent_events: Vec<ClockEventResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CompanyClockEventResponse {
    pub id: String,
    pub user_id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub clock_in: chrono::DateTime<chrono::Utc>,
    pub clock_out: Option<chrono::DateTime<chrono::Utc>>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<db::CompanyClockEventRow> for CompanyClockEventResponse {
    fn from(row: db::CompanyClockEventRow) -> Self {
        let status = if row.is_clocked_in() { "in" } else { "out" }.to_string();

        Self {
            id: row.id,
            user_id: row.user_id,
            first_name: row.first_name,
            last_name: row.last_name,
            email: row.email,
            clock_in: row.clock_in,
            clock_out: row.clock_out,
            status,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CompanyClockEventsResponse {
    pub events: Vec<CompanyClockEventResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// Company DTOs

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CompanyResponse {
    pub id: String,
    pub name: String,
    pub address: String,
    pub logo_id: Option<String>,
    pub logo_url: Option<String>,
    pub data_exported_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deletion_requested_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<db::Company> for CompanyResponse {
    fn from(company: db::Company) -> Self {
        let logo_id = company.logo_id.clone();
        let logo_url = logo_id
            .clone()
            .map(|_id| format!("/api/companies/{}/logo", company.id));
        Self {
            id: company.id,
            name: company.name,
            address: company.address,
            logo_id,
            logo_url,
            data_exported_at: company.data_exported_at,
            deleted_at: company.deleted_at,
            deletion_requested_at: company.deletion_requested_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCompanyRequest {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExportResponse {
    pub message: String,
    pub exported_at: chrono::DateTime<chrono::Utc>,
}

// Custom validator for password policy
pub fn validate_password_policy_attr(password: &str) -> Result<(), validator::ValidationError> {
    if password.len() < 8 {
        return Err(validator::ValidationError::new("password_too_short"));
    }

    if password.len() > 128 {
        return Err(validator::ValidationError::new("password_too_long"));
    }

    let has_uppercase = password.chars().any(char::is_uppercase);
    let has_lowercase = password.chars().any(char::is_lowercase);
    let has_digit = password.chars().any(char::is_numeric);
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    if !has_uppercase {
        return Err(validator::ValidationError::new("password_no_uppercase"));
    }

    if !has_lowercase {
        return Err(validator::ValidationError::new("password_no_lowercase"));
    }

    if !has_digit {
        return Err(validator::ValidationError::new("password_no_digit"));
    }

    if !has_special {
        return Err(validator::ValidationError::new("password_no_special"));
    }

    Ok(())
}

// Custom validator for UUID format
pub fn validate_uuid_format(uuid: &str) -> Result<(), validator::ValidationError> {
    // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
    // Simple check: must be 36 chars with hyphens at positions 8, 13, 18, 23
    if uuid.len() != 36 {
        return Err(validator::ValidationError::new("invalid_uuid_length"));
    }
    
    let bytes = uuid.as_bytes();
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return Err(validator::ValidationError::new("invalid_uuid_format"));
    }
    
    // Check that all other characters are valid hex
    for (i, &b) in bytes.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            continue;
        }
        if !b.is_ascii_hexdigit() {
            return Err(validator::ValidationError::new("invalid_uuid_characters"));
        }
    }
    
    Ok(())
}
