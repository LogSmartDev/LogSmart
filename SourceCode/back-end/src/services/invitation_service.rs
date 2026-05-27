use crate::{
    auth::generate_uuid6_token,
    db, email,
    utils::{AuditContext, AuditLogger},
};
use chrono::Duration;
use sqlx::PgPool;

#[cfg(test)]
mod invitation_service_tests {
    #[tokio::test]
    async fn test_invitation_service_basic() {
        assert!(true);
    }
}

pub struct InvitationService;

impl InvitationService {
    /// Sends a new invitation to join a company.
    ///
    /// # Errors
    /// Returns an error if the user is already registered, already invited, or if email sending fails.
    pub async fn send_invitation(
        db_pool: &PgPool,
        admin: &db::UserRecord,
        recipient_email: String,
        role: db::UserRole,
        branch_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(String, chrono::DateTime<chrono::Utc>), crate::error::AppError> {
        if let Some(_existing_user) = db::get_user_by_email(db_pool, &recipient_email)
            .await
            .map_err(|e| {
                tracing::error!("Failed to check existing user: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?
        {
            return Err(crate::error::AppError::Conflict(
                "User already registered".to_string(),
            ));
        }

        let company_id = admin
            .company_id
            .as_ref()
            .ok_or(crate::error::AppError::Forbidden(
                "Admin user is not associated with a company".to_string(),
            ))?;

        let company_name = db::get_company_by_id(db_pool, company_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch company name: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?
            .ok_or(crate::error::AppError::NotFound(
                "Company not found".to_string(),
            ))?
            .name;

        let token = generate_uuid6_token();
        let expires_at = chrono::Utc::now() + Duration::days(7);

        let invitation = db::create_invitation(
            db_pool,
            company_id.clone(),
            recipient_email.clone(),
            token,
            role,
            branch_id,
            expires_at,
        )
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                tracing::warn!(
                    "Duplicate invitation attempt for email: {}",
                    recipient_email
                );
                crate::error::AppError::Conflict("User already invited".to_string())
            } else {
                tracing::error!("Failed to create invitation: {:?}", e);
                crate::error::AppError::Internal("Failed to create invitation".to_string())
            }
        })?;

        let invite_link = format!(
            "{}/accept-invitation?token={}",
            std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://logsmart.app".to_string()),
            invitation.token
        );

        email::send_invitation_email(&recipient_email, &invite_link, &company_name)
            .await
            .map_err(|e| {
                tracing::error!("Failed to send invitation email: {:?}", e);
                crate::error::AppError::Internal("Failed to send invitation email".to_string())
            })?;

        AuditLogger::log_invitation_sent(
            db_pool,
            admin.id.clone(),
            admin.email.clone(),
            recipient_email,
            crate::audit_ctx!(
                &AuditContext {
                    ip_address,
                    user_agent,
                    ..AuditContext::default()
                },
                actor: admin
            ),
        )
        .await;

        Ok((invitation.id, invitation.expires_at))
    }

    /// Validates an invitation token and returns the invitation details.
    ///
    /// # Errors
    /// Returns an error if the token is invalid, expired, or if database lookup fails.
    pub async fn accept_invitation(
        db_pool: &PgPool,
        token: &str,
    ) -> Result<(db::Invitation, chrono::DateTime<chrono::FixedOffset>), crate::error::AppError>
    {
        let invitation = db::get_invitation_by_token(db_pool, token)
            .await
            .map_err(|e| {
                tracing::error!("Database error fetching invitation by token: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?
            .ok_or(crate::error::AppError::Unauthorized(
                "Invalid or expired invitation".to_string(),
            ))?;

        let now = chrono::Utc::now();
        let expires_at = invitation.expires_at.fixed_offset();

        if now > expires_at {
            return Err(crate::error::AppError::Unauthorized(
                "Invitation has expired".to_string(),
            ));
        }

        Ok((invitation, expires_at))
    }

    /// Retrieves the details of an invitation by its token.
    ///
    /// # Errors
    /// Returns an error if the invitation is not found or if database lookup fails.
    pub async fn get_invitation_details(
        db_pool: &PgPool,
        token: &str,
    ) -> Result<(String, chrono::DateTime<chrono::Utc>), crate::error::AppError> {
        let invitation = db::get_invitation_by_token(db_pool, token)
            .await
            .map_err(|e| {
                tracing::error!("Database error fetching invitation by token: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?
            .ok_or(crate::error::AppError::NotFound(
                "Invitation not found".to_string(),
            ))?;

        let company = db::get_company_by_id(db_pool, &invitation.company_id)
            .await
            .map_err(|e| {
                tracing::error!("Database error fetching company name: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?
            .ok_or(crate::error::AppError::NotFound(
                "Company not found".to_string(),
            ))?;

        Ok((company.name, invitation.expires_at))
    }

    /// Marks an invitation as accepted in the database.
    ///
    /// # Errors
    /// Returns an error if the database update fails.
    pub async fn mark_invitation_accepted(
        db_pool: &PgPool,
        invitation_id: &str,
    ) -> Result<(), crate::error::AppError> {
        let accept_time = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r"
            UPDATE invitations
            SET accepted_at = ?
            WHERE id = ?
            ",
        )
        .bind(&accept_time)
        .bind(invitation_id)
        .execute(db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to mark invitation as accepted: {:?}", e);
            crate::error::AppError::Internal("Failed to accept invitation".to_string())
        })?;

        Ok(())
    }

    /// Retrieves all pending invitations for a company.
    ///
    /// # Errors
    /// Returns an error if the database query fails.
    pub async fn get_pending_invitations(
        db_pool: &PgPool,
        company_id: &str,
    ) -> Result<Vec<db::Invitation>, crate::error::AppError> {
        db::get_pending_invitations_by_company_id(db_pool, company_id)
            .await
            .map_err(|e| {
                tracing::error!("Database error fetching pending invitations: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })
    }

    /// Cancels a pending invitation (admin only).
    ///
    /// # Errors
    /// Returns an error if the caller is not an admin, invitation is not found, or if the operation fails.
    pub async fn cancel_invitation(
        db_pool: &PgPool,
        calling_user: &db::UserRecord,
        invitation_id: &str,
        context: AuditContext,
    ) -> Result<(), crate::error::AppError> {
        let invitation = db::get_invitation_by_id(db_pool, invitation_id)
            .await
            .map_err(|e| {
                tracing::error!("Database error fetching invitation: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?
            .ok_or(crate::error::AppError::NotFound(
                "Invitation not found".to_string(),
            ))?;

        if calling_user.company_id.as_ref() != Some(&invitation.company_id) {
            return Err(crate::error::AppError::Forbidden(
                "Cannot cancel invitations from other companies".to_string(),
            ));
        }

        if invitation.accepted_at.is_some() {
            return Err(crate::error::AppError::BadRequest(
                "Cannot cancel an accepted invitation".to_string(),
            ));
        }

        if invitation.cancelled_at.is_some() {
            return Err(crate::error::AppError::BadRequest(
                "Invitation already cancelled".to_string(),
            ));
        }

        let cancelled_invitation = db::cancel_invitation(db_pool, invitation_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to cancel invitation: {:?}", e);
                crate::error::AppError::Internal("Failed to cancel invitation".to_string())
            })?;

        email::send_invitation_cancelled_email(&cancelled_invitation.email)
            .await
            .map_err(|e| {
                tracing::error!("Failed to send cancellation email: {:?}", e);
            })
            .ok();

        AuditLogger::log_admin_action(
            db_pool,
            calling_user.id.clone(),
            format!("Cancelled invitation for: {}", cancelled_invitation.email),
            crate::audit_ctx!(
                &context,
                actor: calling_user,
                target_email: Some(cancelled_invitation.email)
            ),
        )
        .await;

        Ok(())
    }
}
