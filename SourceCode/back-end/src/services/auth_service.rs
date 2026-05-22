use crate::{
    auth::{generate_uuid6_token, hash_password, validate_password_policy, verify_password},
    db::{self, UserRole},
    error::AppError,
    jwt_manager::JwtManager,
    utils::{AuditContext, AuditLogger},
};
use chrono::Duration;
use sqlx::PgPool;

#[cfg(test)]
mod auth_service_tests {
    #[tokio::test]
    async fn test_auth_service_basic() {
        // Basic test to ensure service compiles
        assert!(true);
    }
}

pub struct AuthService;

impl AuthService {
    /// Registers a new company admin and their company.
    ///
    /// # Errors
    /// Returns an error if database operations, password hashing, or token generation fails.
    ///
    /// # Panics
    /// Panics if JSON serialization of the internal response fails.
    pub async fn register_admin(
        db_pool: &PgPool,
        email: &str,
        first_name: &str,
        last_name: &str,
        password: &str,
        company_name: &str,
        company_address: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(db::UserRecord, String), AppError> {
        let mut tx = db_pool.begin().await.map_err(|e| {
            tracing::error!("Failed to begin transaction: {:?}", e);
            AppError::Internal("Database transaction failed".to_string())
        })?;

        let password_hash = hash_password(password).map_err(|e| {
            tracing::error!("Failed to hash password: {:?}", e);
            AppError::Internal("Password processing failed".to_string())
        })?;

        let company = db::create_company(
            &mut *tx,
            company_name.to_string(),
            company_address.to_string(),
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create company: {:?}", e);
            AppError::Internal("Company creation failed".to_string())
        })?;

        let mut user = db::create_user(
            &mut *tx,
            email.to_string(),
            first_name.to_string(),
            last_name.to_string(),
            Some(password_hash),
            Some(company.id.clone()),
            UserRole::CompanyManager,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create user: {:?}", e);
            AppError::Internal("User creation failed".to_string())
        })?;

        tx.commit().await.map_err(|e| {
            tracing::error!("Failed to commit transaction: {:?}", e);
            AppError::Internal("Transaction commit failed".to_string())
        })?;

        user.company_name = Some(company_name.to_string());

        let token = JwtManager::get_config()
            .generate_token(user.id.as_str(), 24)
            .map_err(|e| {
                tracing::error!("Failed to generate JWT token: {:?}", e);
                AppError::Internal("Token generation failed".to_string())
            })?;

        // Log the registration event
        AuditLogger::log_registration(
            db_pool,
            user.id.clone(),
            email.to_string(),
            company_name.to_string(),
            crate::audit_ctx!(
                &AuditContext {
                    ip_address,
                    user_agent,
                    ..AuditContext::default()
                },
                actor: &user
            ),
        )
        .await;

        Ok((user, token))
    }

    /// Performs user login and returns a JWT token.
    ///
    /// # Errors
    /// Returns an error if credentials are invalid, account is deactivated, or if processing fails.
    ///
    /// # Panics
    /// Panics if JSON serialization of the internal response fails.
    pub async fn login(
        db_pool: &PgPool,
        email: &str,
        password: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<(String, String, UserRole), AppError> {
        let user = db::get_user_by_email(db_pool, email)
            .await
            .map_err(|e| {
                tracing::error!("Database error during login: {:?}", e);
                AppError::Internal("Database error".to_string())
            })?
            .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

        if user.deleted_at.is_some() {
            return Err(AppError::Unauthorized("Account deactivated".to_string()));
        }

        if user.company_deleted_at.is_some() {
            return Err(AppError::Unauthorized(
                "Your company has been deleted. Please contact support.".to_string(),
            ));
        }

        let password_valid = if let Some(password_hash) = &user.password_hash {
            verify_password(password, password_hash).map_err(|e| {
                tracing::error!("Password verification error: {:?}", e);
                AppError::Internal("Authentication failed".to_string())
            })?
        } else {
            return Err(AppError::Unauthorized(
                "OAuth-only account - password login not available".to_string(),
            ));
        };

        if !password_valid {
            AuditLogger::log_login_failed(
                db_pool,
                Some(user.id.clone()),
                email.to_string(),
                crate::audit_ctx!(
                    &AuditContext {
                        ip_address,
                        user_agent,
                        ..AuditContext::default()
                    },
                    actor: &user
                ),
                "Invalid password",
            )
            .await;

            return Err(AppError::Unauthorized("Invalid credentials".to_string()));
        }

        let token = JwtManager::get_config()
            .generate_token(user.id.as_str(), 24)
            .map_err(|e| {
                tracing::error!("Failed to generate login JWT token: {:?}", e);
                AppError::Internal("Failed to generate token".to_string())
            })?;

        // Login time update functionality not yet implemented in db module
        // TODO: Implement update_user_login_time function in db module

        AuditLogger::log_login_success(
            db_pool,
            user.id.clone(),
            email.to_string(),
            crate::audit_ctx!(
                &AuditContext {
                    ip_address,
                    user_agent,
                    ..AuditContext::default()
                },
                actor: &user
            ),
        )
        .await;

        let response = serde_json::json!({
            "message": "Login successful",
            "user_id": user.id,
            "token": token,
            "role": user.get_role().to_string(),
        });

        Ok((
            serde_json::to_string(&response).unwrap(),
            token,
            user.get_role(),
        ))
    }

    /// Requests a password reset for a user.
    ///
    /// # Errors
    /// Returns an error if database operations or email sending fails.
    pub async fn request_password_reset(
        db_pool: &PgPool,
        email: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), AppError> {
        let user = db::get_user_by_email(db_pool, email).await.map_err(|e| {
            tracing::error!("Database error: {:?}", e);
            AppError::Internal("Database error".to_string())
        })?;

        if let Some(user_record) = user {
            let reset_token = generate_uuid6_token();
            let expires_at = chrono::Utc::now() + Duration::hours(24);

            db::create_password_reset_token(
                db_pool,
                user_record.id.clone(),
                reset_token.clone(),
                expires_at,
            )
            .await
            .map_err(|e| {
                tracing::error!("Failed to create password reset token: {:?}", e);
                AppError::Internal("Failed to process password reset request".to_string())
            })?;

            let reset_link = format!(
                "{}/reset-password?token={}",
                "https://logsmart.app", reset_token
            );

            crate::email::send_password_reset_email(&user_record.email, &reset_link)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to send password reset email: {:?}", e);
                    AppError::Internal("Failed to send password reset email".to_string())
                })?;

            AuditLogger::log_password_reset_requested(
                db_pool,
                Some(user_record.id),
                email.to_string(),
                None,
                crate::audit_ctx!(
                    &AuditContext {
                        ip_address,
                        user_agent,
                        ..AuditContext::default()
                    },
                    target_email: Some(email.to_string())
                ),
            )
            .await;
        } else {
            AuditLogger::log_password_reset_requested(
                db_pool,
                None,
                email.to_string(),
                Some("User not found"),
                crate::audit_ctx!(
                    &AuditContext {
                        ip_address,
                        user_agent,
                        ..AuditContext::default()
                    },
                    target_email: Some(email.to_string())
                ),
            )
            .await;
        }

        Ok(())
    }

    /// Validates a password reset token.
    ///
    /// # Errors
    /// Returns an error if the token is invalid, expired, or if database operations fail.
    pub async fn validate_reset_token(db_pool: &PgPool, token: &str) -> Result<bool, AppError> {
        let reset_record = db::get_password_reset_by_token(db_pool, token)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get password reset record: {:?}", e);
                AppError::Internal("Failed to validate reset token".to_string())
            })?
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired reset token".to_string()))?;

        // reset_record returns (reset_id, user_id) tuple
        let (reset_id, _user_id) = reset_record;

        // Mark as used
        db::mark_password_reset_used(db_pool, &reset_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to mark reset token as used: {:?}", e);
                AppError::Internal("Failed to process token".to_string())
            })?;

        Ok(true)
    }

    /// Resets a user's password using a reset token.
    ///
    /// # Errors
    /// Returns an error if the token is invalid, policy validation fails, or database update fails.
    pub async fn reset_password(
        db_pool: &PgPool,
        reset_token: &str,
        new_password: &str,
    ) -> Result<String, AppError> {
        validate_password_policy(new_password).map_err(|e| AppError::BadRequest(e.to_string()))?;

        let (reset_id, user_id) = db::get_password_reset_by_token(db_pool, reset_token)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {:?}", e);
                AppError::Internal("Database error".to_string())
            })?
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired reset token".to_string()))?;

        let password_hash = hash_password(new_password).map_err(|e| {
            tracing::error!("Failed to hash password: {:?}", e);
            AppError::Internal("Failed to process password".to_string())
        })?;

        db::update_user_password(db_pool, &user_id, password_hash)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update password: {:?}", e);
                AppError::Internal("Failed to update password".to_string())
            })?;

        db::mark_password_reset_used(db_pool, &reset_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to mark reset token as used: {:?}", e);
                AppError::Internal("Failed to process request".to_string())
            })?;

        AuditLogger::log_password_reset_completed(
            db_pool,
            user_id.clone(),
            AuditContext {
                target_user_id: Some(user_id.clone()),
                ..AuditContext::default()
            },
        )
        .await;

        Ok(user_id)
    }

    /// Changes a user's password.
    ///
    /// # Errors
    /// Returns an error if current password is incorrect, policy validation fails, or database update fails.
    pub async fn change_password(
        db_pool: &PgPool,
        user_id: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        let user = db::get_user_by_id(db_pool, user_id)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {:?}", e);
                AppError::Internal("Database error".to_string())
            })?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        // Validate current password
        let is_current_valid = if let Some(password_hash) = &user.password_hash {
            verify_password(current_password, password_hash).map_err(|e| {
                tracing::error!("Password verification error: {:?}", e);
                AppError::Internal("Password verification failed".to_string())
            })?
        } else {
            return Err(AppError::BadRequest(
                "OAuth-only account - password change not available".to_string(),
            ));
        };

        if !is_current_valid {
            return Err(AppError::Unauthorized(
                "Current password is incorrect".to_string(),
            ));
        }

        // Validate new password meets requirements
        validate_password_policy(new_password).map_err(|e| AppError::BadRequest(e.to_string()))?;

        let password_hash = hash_password(new_password).map_err(|e| {
            tracing::error!("Failed to hash new password: {:?}", e);
            AppError::Internal("Password processing failed".to_string())
        })?;

        db::update_user_password(db_pool, user_id, password_hash)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update password: {:?}", e);
                AppError::Internal("Failed to update password".to_string())
            })?;

        AuditLogger::log_password_changed(
            db_pool,
            user_id.to_string(),
            user.email.clone(),
            crate::audit_ctx!(&AuditContext::default(), actor: &user),
        )
        .await;

        Ok(())
    }

    /// Verifies user credentials and returns a token and user record.
    ///
    /// # Errors
    /// Returns an error if credentials are invalid or database lookup fails.
    ///
    /// # Panics
    /// Panics if user lookup fails after confirming user existence.
    pub async fn verify_credentials(
        db_pool: &PgPool,
        email: &str,
        password: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(String, db::UserRecord), AppError> {
        let user = db::get_user_by_email(db_pool, email).await.map_err(|e| {
            tracing::error!("Database error during login lookup: {:?}", e);
            AppError::Internal("Database error".to_string())
        })?;

        // If user not found, log the failed attempt before returning error
        if user.is_none() {
            AuditLogger::log_login_failed(
                db_pool,
                None,
                email.to_string(),
                crate::audit_ctx!(
                    &AuditContext {
                        ip_address: ip_address.clone(),
                        user_agent: user_agent.clone(),
                        ..AuditContext::default()
                    },
                    target_email: Some(email.to_string())
                ),
                "User not found",
            )
            .await;
            return Err(AppError::Unauthorized(
                "Invalid email or password".to_string(),
            ));
        }

        let user = user.unwrap();

        if user.deleted_at.is_some() {
            return Err(AppError::Unauthorized("Account deactivated".to_string()));
        }

        if user.company_deleted_at.is_some() {
            return Err(AppError::Unauthorized(
                "Your company has been deleted. Please contact support.".to_string(),
            ));
        }

        if let Some(password_hash) = user.password_hash.as_ref() {
            let password_valid = verify_password(password, password_hash).map_err(|e| {
                tracing::error!("Password verification error: {:?}", e);
                AppError::Internal("Authentication failed".to_string())
            })?;

            if !password_valid {
                AuditLogger::log_login_failed(
                    db_pool,
                    Some(user.id.clone()),
                    email.to_string(),
                    crate::audit_ctx!(
                        &AuditContext {
                            ip_address,
                            user_agent,
                            ..AuditContext::default()
                        },
                        actor: &user
                    ),
                    "Invalid password",
                )
                .await;
                return Err(AppError::Unauthorized(
                    "Invalid email or password".to_string(),
                ));
            }

            let token = JwtManager::get_config()
                .generate_token(user.id.as_str(), 24)
                .map_err(|e| {
                    tracing::error!("Failed to generate login JWT token: {:?}", e);
                    AppError::Internal("Failed to generate token".to_string())
                })?;

            AuditLogger::log_login_success(
                db_pool,
                user.id.clone(),
                email.to_string(),
                crate::audit_ctx!(
                    &AuditContext {
                        ip_address,
                        user_agent,
                        ..AuditContext::default()
                    },
                    actor: &user
                ),
            )
            .await;

            Ok((token, user))
        } else {
            AuditLogger::log_login_failed(
                db_pool,
                Some(user.id.clone()),
                email.to_string(),
                crate::audit_ctx!(
                    &AuditContext {
                        ip_address,
                        user_agent,
                        ..AuditContext::default()
                    },
                    actor: &user
                ),
                "OAuth-only account - password login not available",
            )
            .await;
            Err(AppError::Unauthorized(
                "This account uses OAuth login. Please sign in with Google.".to_string(),
            ))
        }
    }
}
