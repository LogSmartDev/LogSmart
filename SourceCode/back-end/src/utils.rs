use crate::db;
use axum::http::HeaderMap;
use sqlx::PgPool;

#[macro_export]
macro_rules! try_db {
    ($expr:expr, $context:literal) => {
        $expr.await.map_err(|e| {
            tracing::error!(error = ?e, context = $context, "Database error");
            $crate::error::AppError::Internal($context.to_string())
        })
    };
}

pub struct AuditLogger;

#[derive(Debug, Clone, Default)]
pub struct AuditContext {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub actor_role: Option<String>,
    pub company_id: Option<String>,
    pub target_user_id: Option<String>,
    pub target_email: Option<String>,
    pub request_path: Option<String>,
    pub request_method: Option<String>,
}

impl AuditContext {
    pub fn from_request(
        headers: &HeaderMap,
        addr: &std::net::SocketAddr,
        request_path: Option<String>,
        request_method: Option<String>,
    ) -> Self {
        Self {
            ip_address: Some(extract_ip_from_headers_and_addr(headers, addr)),
            user_agent: extract_user_agent(headers),
            request_path,
            request_method,
            ..Self::default()
        }
    }

    pub fn with_actor(mut self, user: &db::UserRecord) -> Self {
        self.actor_role = Some(user.role.to_string());
        self.company_id = user.company_id.clone();
        self
    }

    pub fn with_company_id(mut self, company_id: Option<String>) -> Self {
        self.company_id = company_id;
        self
    }

    pub fn with_target_user_id(mut self, target_user_id: Option<String>) -> Self {
        self.target_user_id = target_user_id;
        self
    }

    pub fn with_target_email(mut self, target_email: Option<String>) -> Self {
        self.target_email = target_email;
        self
    }

    pub fn with_actor_role(mut self, actor_role: Option<String>) -> Self {
        self.actor_role = actor_role;
        self
    }
}

#[macro_export]
macro_rules! audit_ctx {
    ($base:expr $(,)?) => {
        $base.clone()
    };
    ($base:expr, actor: $actor:expr $(, $($rest:tt)*)?) => {{
        let ctx = $crate::audit_ctx!($base $(, $($rest)*)?);
        ctx.with_actor($actor)
    }};
    ($base:expr, actor_role: $actor_role:expr $(, $($rest:tt)*)?) => {{
        let ctx = $crate::audit_ctx!($base $(, $($rest)*)?);
        ctx.with_actor_role($actor_role)
    }};
    ($base:expr, company_id: $company_id:expr $(, $($rest:tt)*)?) => {{
        let ctx = $crate::audit_ctx!($base $(, $($rest)*)?);
        ctx.with_company_id($company_id)
    }};
    ($base:expr, target_user_id: $target_user_id:expr $(, $($rest:tt)*)?) => {{
        let ctx = $crate::audit_ctx!($base $(, $($rest)*)?);
        ctx.with_target_user_id($target_user_id)
    }};
    ($base:expr, target_email: $target_email:expr $(, $($rest:tt)*)?) => {{
        let ctx = $crate::audit_ctx!($base $(, $($rest)*)?);
        ctx.with_target_email($target_email)
    }};
}

impl AuditLogger {
    pub async fn log(
        db: &PgPool,
        event_type: &str,
        user_id: Option<String>,
        email: Option<String>,
        context: AuditContext,
        details: Option<String>,
        success: bool,
    ) {
        if let Err(e) = db::log_security_event(
            db,
            event_type.to_string(),
            user_id,
            email,
            context.ip_address,
            context.user_agent,
            db::SecurityLogMeta {
                actor_role: context.actor_role,
                company_id: context.company_id,
                target_user_id: context.target_user_id,
                target_email: context.target_email,
                request_path: context.request_path,
                request_method: context.request_method,
            },
            details,
            success,
        )
        .await
        {
            tracing::error!("Failed to log security event: {:?}", e);
        }
    }

    pub async fn log_registration(
        db: &PgPool,
        user_id: String,
        email: String,
        company_name: String,
        context: AuditContext,
    ) {
        Self::log(
            db,
            "registration",
            Some(user_id),
            Some(email),
            context,
            Some(format!("Company admin registered: {company_name}")),
            true,
        )
        .await;
    }

    pub async fn log_login_success(
        db: &PgPool,
        user_id: String,
        email: String,
        context: AuditContext,
    ) {
        Self::log(
            db,
            "login_success",
            Some(user_id),
            Some(email),
            context,
            None,
            true,
        )
        .await;
    }

    pub async fn log_login_failed(
        db: &PgPool,
        user_id: Option<String>,
        email: String,
        context: AuditContext,
        reason: &str,
    ) {
        Self::log(
            db,
            "login_failed",
            user_id,
            Some(email),
            context,
            Some(reason.to_string()),
            false,
        )
        .await;
    }

    pub async fn log_invitation_sent(
        db: &PgPool,
        admin_id: String,
        admin_email: String,
        recipient_email: String,
        mut context: AuditContext,
    ) {
        context.target_email = Some(recipient_email.clone());
        Self::log(
            db,
            "invitation_sent",
            Some(admin_id),
            Some(recipient_email),
            context,
            Some(format!("Invitation sent by {admin_email}")),
            true,
        )
        .await;
    }

    pub async fn log_invitation_accepted(
        db: &PgPool,
        user_id: String,
        email: String,
        company_id: String,
        mut context: AuditContext,
    ) {
        context.company_id = Some(company_id.clone());
        Self::log(
            db,
            "invitation_accepted",
            Some(user_id),
            Some(email),
            context,
            Some(format!("Member joined company {company_id}")),
            true,
        )
        .await;
    }

    pub async fn log_profile_updated(
        db: &PgPool,
        user_id: String,
        email: String,
        context: AuditContext,
    ) {
        Self::log(
            db,
            "profile_updated",
            Some(user_id),
            Some(email),
            context,
            None,
            true,
        )
        .await;
    }

    pub async fn log_admin_action(
        db: &PgPool,
        admin_user_id: String,
        action_description: String,
        context: AuditContext,
    ) {
        Self::log(
            db,
            "admin_action",
            Some(admin_user_id),
            None,
            context,
            Some(action_description),
            true,
        )
        .await;
    }

    pub async fn log_password_reset_requested(
        db: &PgPool,
        user_id: Option<String>,
        email: String,
        reason: Option<&str>,
        context: AuditContext,
    ) {
        let is_success = user_id.is_some();
        Self::log(
            db,
            "password_reset_requested",
            user_id,
            Some(email),
            context,
            reason.map(std::string::ToString::to_string),
            is_success,
        )
        .await;
    }

    pub async fn log_password_reset_completed(db: &PgPool, user_id: String, context: AuditContext) {
        Self::log(
            db,
            "password_reset_completed",
            Some(user_id),
            None,
            context,
            None,
            true,
        )
        .await;
    }

    pub async fn log_password_changed(
        db: &PgPool,
        user_id: String,
        email: String,
        context: AuditContext,
    ) {
        Self::log(
            db,
            "password_changed",
            Some(user_id),
            Some(email),
            context,
            Some("User changed their password".to_string()),
            true,
        )
        .await;
    }

    pub async fn log_oauth_login(
        db: &PgPool,
        user_id: String,
        email: String,
        provider: String,
        success: bool,
        context: AuditContext,
    ) {
        Self::log(
            db,
            "oauth_login",
            Some(user_id),
            Some(email),
            context,
            Some(format!("OAuth login via {provider}")),
            success,
        )
        .await;
    }

    pub async fn log_oauth_account_linked(
        db: &PgPool,
        user_id: String,
        email: String,
        provider: String,
        context: AuditContext,
    ) {
        Self::log(
            db,
            "oauth_account_linked",
            Some(user_id),
            Some(email),
            context,
            Some(format!("Linked {provider} account")),
            true,
        )
        .await;
    }

    pub async fn log_oauth_account_unlinked(
        db: &PgPool,
        event_type: String,
        user_id: Option<String>,
        email: Option<String>,
        context: AuditContext,
        details: Option<String>,
        success: bool,
    ) {
        Self::log(db, &event_type, user_id, email, context, details, success).await;
    }
}

pub fn extract_ip_from_headers_and_addr(
    headers: &HeaderMap,
    addr: &std::net::SocketAddr,
) -> String {
    extract_optional_ip_from_headers_and_addr(headers, Some(addr))
        .unwrap_or_else(|| addr.ip().to_string())
}

pub fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(std::string::ToString::to_string)
}

fn first_ip_from_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string)
}

pub fn extract_optional_ip_from_headers_and_addr(
    headers: &HeaderMap,
    addr: Option<&std::net::SocketAddr>,
) -> Option<String> {
    let direct_ip = addr.map(|a| a.ip().to_string());
    first_ip_from_header(headers, "cf-connecting-ip")
        .or_else(|| first_ip_from_header(headers, "true-client-ip"))
        .or_else(|| first_ip_from_header(headers, "x-forwarded-for"))
        .or_else(|| first_ip_from_header(headers, "x-real-ip"))
        .or(direct_ip)
}

pub fn err_internal(msg: &str) -> crate::error::AppError {
    crate::error::AppError::Internal(msg.to_string())
}

pub fn err_not_found(msg: &str) -> crate::error::AppError {
    crate::error::AppError::NotFound(msg.to_string())
}

pub fn err_forbidden(msg: &str) -> crate::error::AppError {
    crate::error::AppError::Forbidden(msg.to_string())
}

pub fn err_bad_request(msg: &str) -> crate::error::AppError {
    crate::error::AppError::BadRequest(msg.to_string())
}

pub fn err_unauthorized(msg: &str) -> crate::error::AppError {
    crate::error::AppError::Unauthorized(msg.to_string())
}

pub fn err_conflict(msg: &str) -> crate::error::AppError {
    crate::error::AppError::Conflict(msg.to_string())
}

pub fn err_too_many_requests(msg: &str) -> crate::error::AppError {
    crate::error::AppError::TooManyRequests(msg.to_string())
}

pub fn err_created<T: serde::Serialize>(
    msg: &str,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    (
        axum::http::StatusCode::CREATED,
        axum::Json(serde_json::json!({ "message": msg })),
    )
}

pub fn err_validation(
    message: &str,
    fields: std::collections::HashMap<String, Vec<String>>,
) -> crate::error::AppError {
    crate::error::AppError::Validation {
        message: message.to_string(),
        fields,
    }
}

/// Converts `validator::ValidationErrors` into user-friendly per-field messages.
pub fn friendly_validation_errors(
    errors: &validator::ValidationErrors,
) -> (String, std::collections::HashMap<String, Vec<String>>) {
    use std::collections::HashMap;

    let mut fields: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_errors: Vec<String> = Vec::new();

    for (field, kind) in errors.errors() {
        let field_label = field_to_label(field.as_ref());
        let mut messages: Vec<String> = Vec::new();

        if let validator::ValidationErrorsKind::Field(field_errors) = kind {
            for err in field_errors {
                let msg = match err.code.as_ref() {
                    "email" => format!("{field_label} has an invalid format"),
                    "length" => {
                        let min = err.params.get("min").and_then(|v| v.as_u64());
                        let max = err.params.get("max").and_then(|v| v.as_u64());
                        match (min, max) {
                            (Some(min), Some(max)) => {
                                format!("{field_label} must be between {min} and {max} characters")
                            }
                            (Some(min), None) => {
                                format!("{field_label} must be at least {min} characters")
                            }
                            (None, Some(max)) => {
                                format!("{field_label} must not exceed {max} characters")
                            }
                            (None, None) => format!("{field_label} has an invalid length"),
                        }
                    }
                    "password_too_short" => "Password must be at least 8 characters".to_string(),
                    "password_too_long" => "Password must not exceed 128 characters".to_string(),
                    "password_no_uppercase" => {
                        "Password must contain at least one uppercase letter".to_string()
                    }
                    "password_no_lowercase" => {
                        "Password must contain at least one lowercase letter".to_string()
                    }
                    "password_no_digit" => "Password must contain at least one digit".to_string(),
                    "password_no_special" => {
                        "Password must contain at least one special character".to_string()
                    }
                    "invalid_uuid_length" => format!("{field_label} must be 36 characters"),
                    "invalid_uuid_format" => {
                        format!("{field_label} has an invalid UUID format (expected hyphens)")
                    }
                    "invalid_uuid_characters" => {
                        format!("{field_label} contains invalid characters")
                    }
                    other => format!("{field_label}: {other}"),
                };
                messages.push(msg.clone());
                all_errors.push(msg);
            }
        }

        fields.insert(field.to_string(), messages);
    }

    let summary = if let [single] = all_errors.as_slice() {
        single.clone()
    } else {
        "Please correct the errors below".to_string()
    };

    (summary, fields)
}

fn field_to_label(field: &str) -> String {
    match field {
        "email" => "Email",
        "password" => "Password",
        "new_password" => "New password",
        "first_name" => "First name",
        "last_name" => "Last name",
        "company_name" => "Company name",
        "company_address" => "Company address",
        "name" => "Name",
        "address" => "Address",
        "branch_id" => "Branch",
        "template_name" => "Template name",
        "version_name" => "Version name",
        "token" => "Token",
        _ => field,
    }
    .to_string()
}

/// Validates that a string is a valid CSS color value.
/// Rejects values containing CSS syntax characters that could be used for injection.
///
/// Valid formats:
/// - Hex colors: #RGB, #RRGGBB, #RRGGBBAA
/// - RGB/RGBA: rgb(...), rgba(...)
/// - Named colors: red, blue, etc.
/// Safe font family values - precompiled list
const SAFE_FONTS: &[&str] = &[
    "system-ui",
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "georgia",
    "times",
    "courier",
    "verdana",
    "arial",
    "helvetica",
];

/// Safe text decoration values - precompiled list
const SAFE_DECORATIONS: &[&str] = &["none", "underline", "overline", "line-through", "blink"];

/// Static regex for hex colors: #RGB or #RRGGBB or #RRGGBBAA
static HEX_COLOR_PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^#[0-9a-fA-F]{3}([0-9a-fA-F]{3})?([0-9a-fA-F]{2})?$")
        .expect("Failed to compile hex color regex")
});

/// Static regex for RGB/RGBA colors: rgb(...) or rgba(...)
static RGB_COLOR_PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^rgba?\s*\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*(,\s*[\d.]+\s*)?\)$")
        .expect("Failed to compile RGB color regex")
});

/// Static regex for named colors: letters only
static NAMED_COLOR_PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^[a-zA-Z]+$").expect("Failed to compile named color regex")
});

/// Static regex for short hex with alpha: #RGBA
static SHORT_HEX_ALPHA_PATTERN: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^#[0-9a-fA-F]{4}$").expect("Failed to compile short hex alpha regex")
    });

pub fn is_valid_css_color(color: &str) -> bool {
    // Empty string is valid (means no custom color)
    if color.trim().is_empty() {
        return true;
    }

    let trimmed = color.trim();

    // Check for dangerous characters that could be used for CSS injection
    // Semicolon, curly braces, backslash (escape), and @ (at-rules)
    if trimmed.contains(';')
        || trimmed.contains('{')
        || trimmed.contains('}')
        || trimmed.contains('\\')
        || trimmed.contains('@')
    {
        return false;
    }

    // Hex color pattern: #RGB or #RRGGBB or #RRGGBBAA
    if HEX_COLOR_PATTERN.is_match(trimmed) {
        return true;
    }

    // RGB/RGBA pattern: rgb(...) or rgba(...)
    if RGB_COLOR_PATTERN.is_match(trimmed) {
        return true;
    }

    // Named colors: letters only (no spaces or special chars)
    if NAMED_COLOR_PATTERN.is_match(trimmed) {
        return true;
    }

    // Hex short format with alpha in older browsers
    if SHORT_HEX_ALPHA_PATTERN.is_match(trimmed) {
        return true;
    }

    // If none of the valid formats match, reject it
    false
}

/// Validates that a font family value is safe (no CSS injection characters).
/// Allows common font family names and safe CSS values.
pub fn is_valid_font_family(font_family: &str) -> bool {
    // Empty string is valid
    if font_family.trim().is_empty() {
        return true;
    }

    let trimmed = font_family.trim();

    // Check for dangerous characters that could be used for CSS injection
    if trimmed.contains(';')
        || trimmed.contains('{')
        || trimmed.contains('}')
        || trimmed.contains('\\')
        || trimmed.contains('@')
    {
        return false;
    }

    // Check if it's in the safe list (case insensitive)
    let lower_font = trimmed.to_lowercase();
    if SAFE_FONTS.iter().any(|f| f == &lower_font) {
        return true;
    }

    // Allow single quoted font names if they don't contain dangerous chars
    if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() > 2 {
        let inside = &trimmed[1..trimmed.len() - 1];
        // Font names with quotes can contain spaces but not dangerous chars
        return !inside.contains(';')
            && !inside.contains('{')
            && !inside.contains('}')
            && !inside.contains('\\')
            && !inside.contains('@');
    }

    // Allow double quoted font names
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 2 {
        let inside = &trimmed[1..trimmed.len() - 1];
        return !inside.contains(';')
            && !inside.contains('{')
            && !inside.contains('}')
            && !inside.contains('\\')
            && !inside.contains('@');
    }

    false
}

/// Validates that a text decoration value is safe (no CSS injection characters).
pub fn is_valid_text_decoration(text_decoration: &str) -> bool {
    // Empty string is valid
    if text_decoration.trim().is_empty() {
        return true;
    }

    let trimmed = text_decoration.trim();

    // Check for dangerous characters that could be used for CSS injection
    if trimmed.contains(';')
        || trimmed.contains('{')
        || trimmed.contains('}')
        || trimmed.contains('\\')
        || trimmed.contains('@')
    {
        return false;
    }

    // Check if it's in the safe list (case insensitive)
    let lower = trimmed.to_lowercase();
    SAFE_DECORATIONS.iter().any(|d| d == &lower)
}

/// Supported input types for template fields
const SUPPORTED_INPUT_TYPES: &[&str] = &["text", "int", "float"];

/// Validates that an input type is supported.
/// Must be one of the canonical types: text, int, float
/// Empty strings are rejected.
pub fn is_valid_input_type(input_type: &str) -> bool {
    let trimmed = input_type.trim();

    // Empty strings are not allowed
    if trimmed.is_empty() {
        return false;
    }

    // Must be in the canonical set (case sensitive)
    SUPPORTED_INPUT_TYPES.contains(&trimmed)
}

/// Validates string length constraints.
/// If provided, both must be:
/// - Non-negative (>= 0)
/// - If both present, `min_length` must be <= `max_length`
///
///  Returns Ok(()) if valid, or descriptive error message if invalid.
pub fn validate_length_constraints(
    min_length: Option<i32>,
    max_length: Option<i32>,
) -> Result<(), String> {
    // Check if min_length is provided and valid
    if let Some(min) = min_length
        && min < 0
    {
        return Err("min_length must be non-negative".to_string());
    }

    // Check if max_length is provided and valid
    if let Some(max) = max_length
        && max < 0
    {
        return Err("max_length must be non-negative".to_string());
    }

    // If both are provided, ensure min <= max
    if let (Some(min), Some(max)) = (min_length, max_length)
        && min > max
    {
        return Err("min_length cannot be greater than max_length".to_string());
    }

    Ok(())
}

/// Infers the MIME type of an image file based on magic bytes.
pub fn infer_content_type(data: &[u8]) -> String {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png".to_string()
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg".to_string()
    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        "image/webp".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_hex_colors() {
        // Valid hex colors
        assert!(is_valid_css_color("#FF0000")); // Red
        assert!(is_valid_css_color("#f00")); // Red short
        assert!(is_valid_css_color("#00FF00")); // Green
        assert!(is_valid_css_color("#0F0")); // Green short
        assert!(is_valid_css_color("#0000FF")); // Blue
        assert!(is_valid_css_color("#00F")); // Blue short
        assert!(is_valid_css_color("#FFFFFF")); // White
        assert!(is_valid_css_color("#FFF")); // White short
        assert!(is_valid_css_color("#000000")); // Black
        assert!(is_valid_css_color("#000")); // Black short
        assert!(is_valid_css_color("#FF0000FF")); // Red with full alpha
        assert!(is_valid_css_color("#F00F")); // Red short with alpha
        assert!(is_valid_css_color("#123456AA")); // With alpha
    }

    #[test]
    fn test_valid_rgb_colors() {
        // Valid RGB colors
        assert!(is_valid_css_color("rgb(255, 0, 0)")); // Red
        assert!(is_valid_css_color("rgb(0, 255, 0)")); // Green
        assert!(is_valid_css_color("rgb(0, 0, 255)")); // Blue
        assert!(is_valid_css_color("rgb(255,0,0)")); // No spaces
        assert!(is_valid_css_color("rgb(  255  ,  0  ,  0  )")); // Extra spaces
    }

    #[test]
    fn test_valid_rgba_colors() {
        // Valid RGBA colors
        assert!(is_valid_css_color("rgba(255, 0, 0, 1)")); // Red fully opaque
        assert!(is_valid_css_color("rgba(255, 0, 0, 0.5)")); // Red semi-transparent
        assert!(is_valid_css_color("rgba(0, 255, 0, 0)")); // Green fully transparent
        assert!(is_valid_css_color("rgba(0,0,255,0.75)")); // No spaces
        assert!(is_valid_css_color("rgba(  100  ,  100  ,  100  ,  0.5  )")); // Extra spaces
    }

    #[test]
    fn test_valid_named_colors() {
        // Valid named colors
        assert!(is_valid_css_color("red"));
        assert!(is_valid_css_color("blue"));
        assert!(is_valid_css_color("green"));
        assert!(is_valid_css_color("white"));
        assert!(is_valid_css_color("black"));
        assert!(is_valid_css_color("transparent"));
        assert!(is_valid_css_color("darkred"));
        assert!(is_valid_css_color("lightblue"));
    }

    #[test]
    fn test_empty_string_is_valid() {
        // Empty string is valid (means no custom color)
        assert!(is_valid_css_color(""));
        assert!(is_valid_css_color("   ")); // Whitespace only
    }

    #[test]
    fn test_malicious_semicolon_injection() {
        // CSS injection via semicolon
        assert!(!is_valid_css_color("red;color:blue"));
        assert!(!is_valid_css_color("#FF0000;display:none"));
        assert!(!is_valid_css_color("rgb(255,0,0);opacity:0"));
        assert!(!is_valid_css_color("red;font-size:100px"));
    }

    #[test]
    fn test_malicious_curly_brace_injection() {
        // CSS injection via curly braces
        assert!(!is_valid_css_color("red{display:none}"));
        assert!(!is_valid_css_color("#FF0000{color:blue}"));
        assert!(!is_valid_css_color("rgb(255,0,0){font-size:100px}"));
    }

    #[test]
    fn test_malicious_at_rule_injection() {
        // CSS injection via @-rules
        assert!(!is_valid_css_color("@import url('evil.css')"));
        assert!(!is_valid_css_color("red@keyframes"));
        assert!(!is_valid_css_color("#FF0000@media"));
    }

    #[test]
    fn test_malicious_backslash_injection() {
        // CSS escape sequences
        assert!(!is_valid_css_color("red\\"));
        assert!(!is_valid_css_color("red\\000041"));
        assert!(!is_valid_css_color("#FF0000\\20display\\3Anone"));
    }

    #[test]
    fn test_malicious_comment_injection() {
        // Note: Comments don't have the injection characters, but testing edge cases
        assert!(!is_valid_css_color("red/**/color:blue")); // Comment with extra chars won't match patterns
        assert!(!is_valid_css_color("rgb(255,0,0)/*comment*/")); // This will fail because of special chars
    }

    #[test]
    fn test_invalid_color_formats() {
        // Invalid color formats
        assert!(!is_valid_css_color("123456")); // No # for hex
        assert!(!is_valid_css_color("#GGGGGG")); // Invalid hex characters
        assert!(!is_valid_css_color("#FF")); // Too few hex digits (2)
        assert!(!is_valid_css_color("#FF00000")); // Invalid hex length (7 digits)
        assert!(!is_valid_css_color("rgb(-1, 0, 0)")); // Negative values (starts with -)
        assert!(!is_valid_css_color("rgb(255, 0)")); // Missing parameter
        assert!(!is_valid_css_color("rgb(255 0 0)")); // Space separator instead of comma
        assert!(!is_valid_css_color("hsl(120, 100%, 50%)")); // HSL not supported in our validation
        assert!(!is_valid_css_color("rgba 255 0 0 1")); // Invalid syntax
        assert!(!is_valid_css_color("red blue")); // Multiple colors
        assert!(!is_valid_css_color("red!")); // Special characters
    }

    #[test]
    fn test_case_insensitive_colors() {
        // Color names should be case insensitive in CSS, but our validator accepts any letters
        assert!(is_valid_css_color("RED"));
        assert!(is_valid_css_color("Red"));
        assert!(is_valid_css_color("rEd"));
        assert!(is_valid_css_color("#ff0000")); // Hex lowercase
        assert!(is_valid_css_color("#FF0000")); // Hex uppercase
        assert!(is_valid_css_color("#Ff00Ff")); // Hex mixed case
    }

    #[test]
    fn test_valid_font_families() {
        // Safe font family values
        assert!(is_valid_font_family("system-ui"));
        assert!(is_valid_font_family("serif"));
        assert!(is_valid_font_family("sans-serif"));
        assert!(is_valid_font_family("monospace"));
        assert!(is_valid_font_family("cursive"));
        assert!(is_valid_font_family("fantasy"));
        assert!(is_valid_font_family("georgia"));
        assert!(is_valid_font_family("times"));
        assert!(is_valid_font_family("courier"));
        assert!(is_valid_font_family("verdana"));
        assert!(is_valid_font_family("arial"));
        assert!(is_valid_font_family("helvetica"));
        assert!(is_valid_font_family("Georgia")); // Case insensitive
        assert!(is_valid_font_family("ARIAL"));
        assert!(is_valid_font_family("")); // Empty is valid
        assert!(is_valid_font_family("   ")); // Whitespace only is valid
    }

    #[test]
    fn test_quoted_font_families() {
        // Quoted font families are allowed
        assert!(is_valid_font_family("'Custom Font'"));
        assert!(is_valid_font_family("'Times New Roman'"));
        assert!(is_valid_font_family("\"Custom Font\""));
        assert!(is_valid_font_family("\"Courier New\""));
    }

    #[test]
    fn test_malicious_font_families() {
        // Font families with injection characters should be rejected
        assert!(!is_valid_font_family("serif;color:red"));
        assert!(!is_valid_font_family("arial{display:none}"));
        assert!(!is_valid_font_family("times\\000041"));
        assert!(!is_valid_font_family("@import"));
        assert!(!is_valid_font_family("'Custom';display:none")); // Injection in quoted
    }

    #[test]
    fn test_valid_text_decorations() {
        // Safe text decoration values
        assert!(is_valid_text_decoration("none"));
        assert!(is_valid_text_decoration("underline"));
        assert!(is_valid_text_decoration("overline"));
        assert!(is_valid_text_decoration("line-through"));
        assert!(is_valid_text_decoration("blink"));
        assert!(is_valid_text_decoration("None")); // Case insensitive
        assert!(is_valid_text_decoration("UNDERLINE"));
        assert!(is_valid_text_decoration("")); // Empty is valid
        assert!(is_valid_text_decoration("   ")); // Whitespace only is valid
    }

    #[test]
    fn test_malicious_text_decorations() {
        // Text decorations with injection characters should be rejected
        assert!(!is_valid_text_decoration("none;color:red"));
        assert!(!is_valid_text_decoration("underline{display:none}"));
        assert!(!is_valid_text_decoration("line-through\\000041"));
        assert!(!is_valid_text_decoration("@keyframes"));
        assert!(!is_valid_text_decoration("invalid-value")); // Not a valid decoration
    }

    #[test]
    fn test_valid_input_types() {
        // Canonical input types
        assert!(is_valid_input_type("text"));
        assert!(is_valid_input_type("int"));
        assert!(is_valid_input_type("float"));
    }

    #[test]
    fn test_invalid_input_types() {
        // Invalid input types
        assert!(!is_valid_input_type("")); // Empty
        assert!(!is_valid_input_type("   ")); // Whitespace only
        assert!(!is_valid_input_type("email")); // Not supported
        assert!(!is_valid_input_type("number")); // Not supported
        assert!(!is_valid_input_type("TEXT")); // Case sensitive
        assert!(!is_valid_input_type("INT")); // Case sensitive
        assert!(!is_valid_input_type("FLOAT")); // Case sensitive
        assert!(!is_valid_input_type("int;select")); // Injection attempt
    }

    #[test]
    fn test_valid_length_constraints() {
        // Valid combinations
        assert!(validate_length_constraints(None, None).is_ok());
        assert!(validate_length_constraints(Some(0), None).is_ok());
        assert!(validate_length_constraints(None, Some(100)).is_ok());
        assert!(validate_length_constraints(Some(0), Some(100)).is_ok());
        assert!(validate_length_constraints(Some(10), Some(10)).is_ok());
        assert!(validate_length_constraints(Some(5), Some(100)).is_ok());
    }

    #[test]
    fn test_invalid_length_constraints() {
        // Negative min_length
        assert!(validate_length_constraints(Some(-1), None).is_err());
        assert!(validate_length_constraints(Some(-1), Some(100)).is_err());

        // Negative max_length
        assert!(validate_length_constraints(None, Some(-1)).is_err());
        assert!(validate_length_constraints(Some(0), Some(-1)).is_err());

        // min > max
        assert!(validate_length_constraints(Some(100), Some(10)).is_err());
        assert!(validate_length_constraints(Some(5), Some(4)).is_err());
    }

    #[test]
    fn test_length_constraint_error_messages() {
        // Verify specific error messages for debugging
        let result = validate_length_constraints(Some(-1), None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "min_length must be non-negative");

        let result = validate_length_constraints(None, Some(-5));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "max_length must be non-negative");

        let result = validate_length_constraints(Some(100), Some(50));
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "min_length cannot be greater than max_length"
        );
    }
}
