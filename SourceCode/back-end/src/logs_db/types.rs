use std::fmt::Display;

use mongodb::bson::Uuid;
use schemars::JsonSchema;
use utoipa::ToSchema;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub struct TemplateFieldProps {
    pub text: Option<String>,
    pub size: Option<String>,
    pub weight: Option<String>,
    pub value: Option<String>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub unit: Option<String>,
    pub selected: Option<String>,
    pub options: Option<Vec<String>>,
    pub editable: Option<bool>,
    pub placeholder: Option<String>,
    pub font_family: Option<String>,
    pub text_decoration: Option<String>,
    pub color: Option<String>,
    pub required: Option<bool>,
    pub max_length: Option<i32>,
    pub min_length: Option<i32>,
    pub input_type: Option<String>,
}

impl Default for TemplateFieldProps {
    fn default() -> Self {
        TemplateFieldProps {
            text: None,
            size: None,
            weight: None,
            value: None,
            min: None,
            max: None,
            unit: None,
            selected: None,
            options: None,
            editable: Some(true),
            placeholder: None,
            font_family: None,
            text_decoration: None,
            color: None,
            required: Some(false),
            max_length: None,
            min_length: None,
            input_type: Some("text".to_string()),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub struct TemplateField {
    pub field_type: String,
    pub position: Position,
    pub props: TemplateFieldProps,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct Schedule {
    pub frequency: Frequency,
    pub days_of_week: Option<Vec<u8>>,
    pub day_of_week: Option<u8>,
    pub day_of_month: Option<u8>,
    pub month_of_year: Option<u8>,
    #[serde(default)]
    pub available_from_time: Option<String>,
    #[serde(default)]
    pub due_at_time: Option<String>,
}

pub type TemplateLayout = Vec<TemplateField>;

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct TemplateDocument {
    pub template_name: String,
    pub template_layout: TemplateLayout,
    pub company_id: String,
    pub branch_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub schedule: Schedule,
    pub created_by: Uuid,
    #[serde(default = "default_version")]
    pub version: u16,
    pub version_name: Option<String>,
}

impl TemplateDocument {
    #[must_use]
    pub fn is_company_wide(&self) -> bool {
        self.branch_id.is_none()
    }
}

fn default_version() -> u16 {
    1
}

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct TemplateVersionDocument {
    pub template_name: String,
    pub company_id: String,
    pub branch_id: Option<String>,
    pub version: u16,
    pub version_name: Option<String>,
    pub template_layout: TemplateLayout,
    pub schedule: Schedule,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct LogEntry {
    pub entry_id: String,
    pub template_name: String,
    pub company_id: String,
    pub branch_id: Option<String>,
    pub user_id: String,
    pub entry_data: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: LogStatus,
    pub period: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ReportRunDocument {
    pub report_id: String,
    pub user_id: String,
    pub company_id: String,
    pub name: Option<String>,
    pub params: crate::dto::ReportRunParams,
    #[serde(default)]
    pub params_key: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub use_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityStatus {
    NotAvailable,
    Available,
    Overdue,
}

impl Display for AvailabilityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AvailabilityStatus::NotAvailable => write!(f, "not_available"),
            AvailabilityStatus::Available => write!(f, "available"),
            AvailabilityStatus::Overdue => write!(f, "overdue"),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum LogStatus {
    #[default]
    Draft,
    Submitted,
    Reviewed,
    Approved,
    Overdue,
}

impl std::str::FromStr for LogStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(LogStatus::Draft),
            "submitted" => Ok(LogStatus::Submitted),
            "reviewed" => Ok(LogStatus::Reviewed),
            "approved" => Ok(LogStatus::Approved),
            "overdue" => Ok(LogStatus::Overdue),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for LogStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogStatus::Draft => write!(f, "draft"),
            LogStatus::Submitted => write!(f, "submitted"),
            LogStatus::Reviewed => write!(f, "reviewed"),
            LogStatus::Approved => write!(f, "approved"),
            LogStatus::Overdue => write!(f, "overdue"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodValidationError {
    FormatInvalid,
    DueDateInFuture,
    WeekdayNotAllowed,
    BeforeTemplateCreation,
}
