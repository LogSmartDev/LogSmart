use crate::middleware::{AnyAuthUser, AuditRequestContext};
use crate::services::UserService;
use crate::utils::{
    err_bad_request, err_internal, err_not_found, err_unauthorized,
    extract_ip_from_headers_and_addr, extract_user_agent,
};
use crate::{
    AppState,
    auth::{validate_email, validate_password_policy},
    db,
    dto::{
        AuthResponse, ErrorResponse, JwtVerifyResponse, LoginRequest, PasswordResetResponse,
        RegisterRequest, RequestPasswordResetRequest, ResetPasswordRequest, UpdateProfileRequest,
        UserResponse, VerifyTokenRequest,
    },
    jwt_manager::JwtManager,
    services,
    utils::AuditLogger,
};
use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{self, HeaderMap, StatusCode, header::SET_COOKIE},
    response::IntoResponse,
};

#[utoipa::path(
    post,
    path = "/auth/verify",
    request_body = VerifyTokenRequest,
    responses(
        (status = 200, description = "Token is valid", body = JwtVerifyResponse),
        (status = 401, description = "Invalid or expired token", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
    ),
    tag = "Authentication"
)]
/// Verifies the validity of a JWT token.
///
/// # Errors
/// Returns an error if the token is invalid, expired, or if there's a database error.
pub async fn verify_token(
    State(state): State<AppState>,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    Json(payload): Json<VerifyTokenRequest>,
) -> Result<Json<JwtVerifyResponse>, crate::error::AppError> {
    let jwt_config = JwtManager::get_config();
    let claims = match jwt_config.validate_token(&payload.token) {
        Ok(claims) => claims,
        Err(e) => {
            tracing::error!("Token verification failed: {:?}", e);
            AuditLogger::log(
                &state.postgres,
                "token_verify_failed",
                None,
                None,
                crate::audit_ctx!(&audit_ctx),
                Some("Invalid or expired token".to_string()),
                false,
            )
            .await;
            return Err(err_unauthorized("Invalid or expired token"));
        }
    };

    let user = if let Some(user) = state.user_cache.get(&claims.user_id).await {
        user
    } else {
        let user = db::get_user_by_id(&state.postgres, &claims.user_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    "Database error fetching user during token verification: {:?}",
                    e
                );
                err_internal("Database error")
            })?
            .ok_or(err_not_found("User not found"))?;
        state
            .user_cache
            .insert(claims.user_id.clone(), user.clone())
            .await;
        user
    };

    if user.company_deleted_at.is_some() {
        AuditLogger::log(
            &state.postgres,
            "token_verify_failed",
            Some(user.id.clone()),
            Some(user.email.clone()),
            crate::audit_ctx!(&audit_ctx, actor: &user),
            Some("Company deleted".to_string()),
            false,
        )
        .await;
        return Err(err_unauthorized(
            "Your company has been deleted. Please contact support.",
        ));
    }

    AuditLogger::log(
        &state.postgres,
        "token_verify_success",
        Some(user.id.clone()),
        Some(user.email.clone()),
        crate::audit_ctx!(&audit_ctx, actor: &user),
        None,
        true,
    )
    .await;

    Ok(Json(JwtVerifyResponse { email: user.email }))
}

#[utoipa::path(
    post,
    path = "/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "Company admin registered successfully", body = AuthResponse),
        (status = 400, description = "Invalid request data", body = ErrorResponse),
        (status = 409, description = "Email already exists", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse),
    ),
    tag = "Authentication"
)]
/// Registers a new company and its administrative user.
///
/// # Errors
/// Returns an error if validation fails, the email already exists, or registration fails.
pub async fn register_company_admin(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    let _timer = crate::metrics::RequestTimer::new();
    state.metrics.increment_total_requests();

    let ip_address = extract_ip_from_headers_and_addr(&headers, &addr);
    let user_agent = extract_user_agent(&headers);

    if payload.email.is_empty()
        || payload.first_name.is_empty()
        || payload.last_name.is_empty()
        || payload.password.is_empty()
        || payload.company_name.is_empty()
        || payload.company_address.is_empty()
    {
        return Err(crate::error::AppError::BadRequest(
            "Missing required fields".to_string(),
        ));
    }

    if let Err(e) = validate_email(&payload.email) {
        return Err(crate::error::AppError::BadRequest(e.to_string()));
    }

    if let Err(e) = validate_password_policy(&payload.password) {
        return Err(crate::error::AppError::BadRequest(e.to_string()));
    }

    if db::get_user_by_email(&state.postgres, &payload.email)
        .await
        .map_err(|e| {
            tracing::error!("Database error checking existing user: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?
        .is_some()
    {
        return Err(crate::error::AppError::Conflict(
            "Email already exists".to_string(),
        ));
    }

    let (user_record, token): (db::UserRecord, String) = services::AuthService::register_admin(
        &state.postgres,
        &payload.email,
        &payload.first_name,
        &payload.last_name,
        &payload.password,
        &payload.company_name,
        &payload.company_address,
        Some(ip_address),
        user_agent,
    )
    .await?;

    state.metrics.increment_registrations();
    state.metrics.increment_successful_requests();
    tracing::info!("Registration successful");

    let cookie_domain = std::env::var("COOKIE_DOMAIN").unwrap_or_default();
    let domain_attr = if cookie_domain.is_empty() {
        String::new()
    } else {
        format!("; Domain={cookie_domain}")
    };

    let cookie = format!(
        "ls-token={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}{}",
        token,
        60 * 60 * 24 * 7,
        domain_attr
    );

    let mut response = (
        StatusCode::CREATED,
        Json(AuthResponse {
            token: token.clone(),
            user: user_record.into(),
        }),
    )
        .into_response();

    response.headers_mut().insert(
        SET_COOKIE,
        http::HeaderValue::from_str(&cookie).map_err(|e| {
            tracing::error!("Failed to set registration cookie: {:?}", e);
            err_internal("Failed to set authentication cookie")
        })?,
    );

    Ok(response)
}

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 400, description = "Invalid request data", body = ErrorResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse),
    ),
    tag = "Authentication"
)]
/// Authenticates a user and returns a JWT token.
///
/// # Errors
/// Returns an error if credentials are invalid or if authentication fails.
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    let _timer = crate::metrics::RequestTimer::new();
    state.metrics.increment_total_requests();
    state.metrics.increment_login_attempts();

    let ip_address = extract_ip_from_headers_and_addr(&headers, &addr);
    let user_agent = extract_user_agent(&headers);

    if payload.email.is_empty() || payload.password.is_empty() {
        return Err(err_bad_request("Missing email or password"));
    }

    let (token, user): (String, db::UserRecord) = services::AuthService::verify_credentials(
        &state.postgres,
        &payload.email,
        &payload.password,
        Some(ip_address),
        user_agent,
    )
    .await
    .inspect_err(|_e| {
        state.metrics.increment_login_failures();
        state.metrics.increment_failed_requests();
    })?;

    state.metrics.increment_login_successes();
    state.metrics.increment_successful_requests();
    tracing::info!("Login successful for user: {}", user.id);

    let cookie_domain = std::env::var("COOKIE_DOMAIN").unwrap_or_default();
    let domain_attr = if cookie_domain.is_empty() {
        String::new()
    } else {
        format!("; Domain={cookie_domain}")
    };

    let cookie = format!(
        "ls-token={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}{}",
        token,
        60 * 60 * 24 * 7,
        domain_attr
    );

    let mut response = Json(AuthResponse {
        token: token.clone(),
        user: user.into(),
    })
    .into_response();

    response.headers_mut().insert(
        SET_COOKIE,
        http::HeaderValue::from_str(&cookie).map_err(|e| {
            tracing::error!("Failed to set login cookie: {:?}", e);
            err_internal("Failed to set authentication cookie")
        })?,
    );

    Ok(response)
}

#[utoipa::path(
    get,
    path = "/auth/me",
    responses(
        (status = 200, description = "Current user information", body = UserResponse),
        (status = 401, description = "Unauthorized - invalid or missing token", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
/// Retrieves the profile information of the currently authenticated user.
///
/// # Errors
/// Returns an error if the user is not found or if there's a database error.
pub async fn get_current_user(
    AnyAuthUser(_claims, user): AnyAuthUser,
) -> Result<Json<UserResponse>, crate::error::AppError> {
    Ok(Json(user.into()))
}

#[utoipa::path(
    put,
    path = "/auth/profile",
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Profile updated successfully", body = UserResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "Authentication"
)]
/// Updates the profile information of the currently authenticated user.
///
/// # Errors
/// Returns an error if validation fails or the update operation fails.
pub async fn update_profile(
    AnyAuthUser(claims, _user): AnyAuthUser,
    AuditRequestContext(audit_ctx): AuditRequestContext,
    State(state): State<AppState>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<UserResponse>, crate::error::AppError> {
    if payload.first_name.is_empty() || payload.last_name.is_empty() {
        return Err(err_bad_request("First name and last name cannot be empty"));
    }
    let user = UserService::update_profile(
        &state.postgres,
        &claims.user_id,
        payload.first_name,
        payload.last_name,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to update profile: {:?}", e);
        e
    })?;

    // Invalidate cache
    state.user_cache.invalidate(&claims.user_id).await;

    AuditLogger::log_profile_updated(
        &state.postgres,
        claims.user_id,
        user.email.clone(),
        crate::audit_ctx!(&audit_ctx, actor: &user),
    )
    .await;

    Ok(Json(user.into()))
}

#[utoipa::path(
    post,
    path = "/auth/password/request-reset",
    request_body = RequestPasswordResetRequest,
    responses(
        (status = 200, description = "Password reset email sent", body = PasswordResetResponse),
        (status = 400, description = "Invalid email", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    tag = "Authentication"
)]
/// Initiates a password reset process by sending an email.
///
/// # Errors
/// Returns an error if the email is invalid or the request fails.
pub async fn request_password_reset(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<RequestPasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, crate::error::AppError> {
    let _timer = crate::metrics::RequestTimer::new();
    state.metrics.increment_total_requests();

    if payload.email.is_empty() {
        return Err(err_bad_request("Email is required"));
    }

    if let Err(e) = validate_email(&payload.email) {
        return Err(err_bad_request(&e.to_string()));
    }

    let ip_address = extract_ip_from_headers_and_addr(&headers, &addr);
    let user_agent = extract_user_agent(&headers);

    services::AuthService::request_password_reset(
        &state.postgres,
        &payload.email,
        Some(ip_address),
        user_agent,
    )
    .await
    .inspect_err(|_e| {
        state.metrics.increment_failed_requests();
    })?;

    state.metrics.increment_successful_requests();

    Ok(Json(PasswordResetResponse {
        message: "If an account exists with this email, a password reset link has been sent."
            .to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/auth/password/reset",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset successfully", body = PasswordResetResponse),
        (status = 401, description = "Invalid or expired token", body = ErrorResponse),
        (status = 400, description = "Password validation failed", body = ErrorResponse),
        (status = 500, description = "Server error", body = ErrorResponse),
    ),
    tag = "Authentication"
)]
/// Resets a user's password using a valid reset token.
///
/// # Errors
/// Returns an error if the token is invalid/expired or the operation fails.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<Json<PasswordResetResponse>, crate::error::AppError> {
    let user_id = services::AuthService::reset_password(
        &state.postgres,
        &payload.token,
        &payload.new_password,
    )
    .await
    .map_err(|e| {
        tracing::error!("Password reset failed: {:?}", e);
        e
    })?;

    // Invalidate cache
    state.user_cache.invalidate(&user_id).await;

    state.metrics.increment_successful_requests();

    Ok(Json(PasswordResetResponse {
        message: "Password has been reset successfully.".to_string(),
    }))
}
