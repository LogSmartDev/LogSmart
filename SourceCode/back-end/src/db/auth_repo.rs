use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

/// Creates a new invitation for a user to join a company.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_invitation(
    pool: &PgPool,
    company_id: String,
    email: String,
    token: String,
    role: UserRole,
    branch_id: Option<String>,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<Invitation> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO invitations (id, company_id, email, token, role, branch_id, created_at, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ",
    )
    .bind(&id)
    .bind(&company_id)
    .bind(&email)
    .bind(&token)
    .bind(&role)
    .bind(&branch_id)
    .bind(now)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(Invitation {
        id,
        company_id,
        email,
        token,
        role,
        branch_id,
        created_at: now,
        expires_at,
        accepted_at: None,
        cancelled_at: None,
    })
}

/// Retrieves an invitation by its token.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_invitation_by_token(pool: &PgPool, token: &str) -> Result<Option<Invitation>> {
    let invitation = sqlx::query_as::<_, Invitation>(
        r"
        SELECT id, company_id, email, token, role, branch_id, created_at, expires_at, accepted_at, cancelled_at
        FROM invitations
        WHERE token = $1 AND accepted_at IS NULL AND cancelled_at IS NULL
        ",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;

    Ok(invitation)
}

/// Marks an invitation as accepted.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn accept_invitation(pool: &PgPool, invitation_id: &str) -> Result<()> {
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        UPDATE invitations
        SET accepted_at = $1
        WHERE id = $2
        ",
    )
    .bind(now)
    .bind(invitation_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Cancels a pending invitation.
///
/// # Errors
/// Returns an error if database update fails or invitation not found.
pub async fn cancel_invitation(pool: &PgPool, invitation_id: &str) -> Result<Invitation> {
    let now = chrono::Utc::now();

    let invitation = sqlx::query_as::<_, Invitation>(
        r"
        UPDATE invitations
        SET cancelled_at = $1
        WHERE id = $2 AND accepted_at IS NULL AND cancelled_at IS NULL
        RETURNING id, company_id, email, token, role, branch_id, created_at, expires_at, accepted_at, cancelled_at
        ",
    )
    .bind(now)
    .bind(invitation_id)
    .fetch_one(pool)
    .await?;

    Ok(invitation)
}

/// Retrieves an invitation by its ID.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_invitation_by_id(
    pool: &PgPool,
    invitation_id: &str,
) -> Result<Option<Invitation>> {
    let invitation = sqlx::query_as::<_, Invitation>(
        r"
        SELECT id, company_id, email, token, role, branch_id, created_at, expires_at, accepted_at, cancelled_at
        FROM invitations
        WHERE id = $1
        ",
    )
    .bind(invitation_id)
    .fetch_optional(pool)
    .await?;

    Ok(invitation)
}

/// Accepts an invitation and creates a new user in a single transaction.
///
/// # Errors
/// Returns an error if the transaction fails, which can happen if database operations fail.
pub async fn accept_invitation_with_user_creation(
    pool: &PgPool,
    invitation_id: &str,
    email: &str,
    first_name: String,
    last_name: String,
    password_hash: String,
    company_id: &str,
    role: UserRole,
    branch_id: Option<String>,
) -> Result<UserRecord> {
    let mut tx = pool.begin().await?;

    let user_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO users (id, email, first_name, last_name, password_hash, company_id, branch_id, role, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ",
    )
    .bind(&user_id)
    .bind(email)
    .bind(&first_name)
    .bind(&last_name)
    .bind(&password_hash)
    .bind(company_id)
    .bind(&branch_id)
    .bind(&role)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r"
        UPDATE invitations
        SET accepted_at = $1
        WHERE id = $2
        ",
    )
    .bind(now)
    .bind(invitation_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(UserRecord {
        id: user_id,
        email: email.to_string(),
        first_name,
        last_name,
        password_hash: Some(password_hash),
        company_id: Some(company_id.to_string()),
        branch_id,
        company_name: None,
        profile_picture_id: None,
        role,
        created_at: now,
        deleted_at: None,
        oauth_provider: None,
        oauth_subject: None,
        oauth_picture: None,
        company_deleted_at: None,
    })
}

/// Creates a new passkey for a user.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_passkey(
    pool: &PgPool,
    user_id: &str,
    credential_id: String,
    public_key: String,
    name: String,
) -> Result<Passkey> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO passkeys (id, user_id, credential_id, public_key, counter, name, created_at)
        VALUES ($1, $2, $3, $4, 0, $5, $6)
        ",
    )
    .bind(&id)
    .bind(user_id)
    .bind(&credential_id)
    .bind(&public_key)
    .bind(&name)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(Passkey {
        id,
        user_id: user_id.to_string(),
        credential_id,
        public_key,
        counter: 0,
        name,
        created_at: now,
        last_used_at: None,
    })
}

/// Retrieves all passkeys for a specific user.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_passkeys_by_user(pool: &PgPool, user_id: &str) -> Result<Vec<Passkey>> {
    let passkeys = sqlx::query_as::<_, Passkey>(
        r"
        SELECT id, user_id, credential_id, public_key, counter, name, created_at, last_used_at
        FROM passkeys
        WHERE user_id = $1
        ORDER BY created_at DESC
        ",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(passkeys)
}

/// Retrieves a passkey by its credential ID.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_passkey_by_credential_id(
    pool: &PgPool,
    credential_id: &str,
) -> Result<Option<Passkey>> {
    let passkey = sqlx::query_as::<_, Passkey>(
        r"
        SELECT id, user_id, credential_id, public_key, counter, name, created_at, last_used_at
        FROM passkeys
        WHERE credential_id = $1
        ",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await?;

    Ok(passkey)
}

/// Updates a passkey's usage counter and last used timestamp.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn update_passkey_usage(pool: &PgPool, id: &str, counter: i64) -> Result<()> {
    sqlx::query(
        r"
        UPDATE passkeys
        SET counter = $1, last_used_at = $2
        WHERE id = $3
        ",
    )
    .bind(counter)
    .bind(chrono::Utc::now())
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update passkey usage: {}", e))?;
    Ok(())
}

/// Deletes a passkey for a specific user.
///
/// # Errors
/// Returns an error if database deletion fails.
pub async fn delete_passkey(pool: &PgPool, id: &str, user_id: &str) -> Result<()> {
    let result = sqlx::query(
        r"
        DELETE FROM passkeys
        WHERE id = $1 AND user_id = $2
        ",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Passkey not found"));
    }

    Ok(())
}

/// Creates a password reset token for a user.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_password_reset_token(
    pool: &PgPool,
    user_id: String,
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO password_resets (id, user_id, token, created_at, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(&id)
    .bind(&user_id)
    .bind(&token)
    .bind(now)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(id)
}

/// Retrieves a password reset record by its token.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_password_reset_by_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<(String, String)>> {
    let result = sqlx::query_as::<_, (String, String)>(
        r"
        SELECT id, user_id
        FROM password_resets
        WHERE token = $1 AND used_at IS NULL AND expires_at > now()
        ",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// Marks a password reset token as used.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn mark_password_reset_used(pool: &PgPool, reset_id: &str) -> Result<()> {
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        UPDATE password_resets
        SET used_at = $1
        WHERE id = $2
        ",
    )
    .bind(now)
    .bind(reset_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Creates a new passkey session (challenge).
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_passkey_session(
    pool: &PgPool,
    id: &str,
    session_type: &str,
    user_id: Option<String>,
    challenge: String,
    meta: Option<String>,
) -> Result<()> {
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);

    sqlx::query(
        r"
        INSERT INTO passkey_sessions (id, session_type, user_id, challenge, meta, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ",
    )
    .bind(id)
    .bind(session_type)
    .bind(user_id)
    .bind(challenge)
    .bind(meta)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(())
}

/// Retrieves a passkey session by its ID if it hasn't expired.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_passkey_session(pool: &PgPool, id: &str) -> Result<Option<PasskeySession>> {
    let session = sqlx::query_as::<_, PasskeySession>(
        r"
        SELECT id, session_type, user_id, challenge, meta, created_at, expires_at
        FROM passkey_sessions
        WHERE id = $1 AND expires_at > NOW()
        ",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(session)
}

/// Deletes a passkey session.
///
/// # Errors
/// Returns an error if database deletion fails.
pub async fn delete_passkey_session(pool: &PgPool, id: &str) -> Result<()> {
    sqlx::query(
        r"
        DELETE FROM passkey_sessions
        WHERE id = $1
        ",
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Retrieves all pending invitations for a specific company.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_pending_invitations_by_company_id(
    pool: &PgPool,
    company_id: &str,
) -> Result<Vec<Invitation>> {
    let invitations = sqlx::query_as::<_, Invitation>(
        r"
        SELECT id, company_id, email, token, role, branch_id, created_at, expires_at, accepted_at, cancelled_at
        FROM invitations
        WHERE company_id = $1 AND accepted_at IS NULL AND cancelled_at IS NULL AND expires_at > now()
        ",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    Ok(invitations)
}
