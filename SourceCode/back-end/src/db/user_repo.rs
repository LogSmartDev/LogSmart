use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;
use super::USER_SELECT_COLUMNS;

/// Creates a new user in the database.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_user<'a, E>(
    executor: E,
    email: String,
    first_name: String,
    last_name: String,
    password_hash: Option<String>,
    company_id: Option<String>,
    role: UserRole,
) -> Result<UserRecord>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO users (id, email, first_name, last_name, password_hash, company_id, role, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ",
    )
    .bind(&id)
    .bind(&email)
    .bind(&first_name)
    .bind(&last_name)
    .bind(&password_hash)
    .bind(&company_id)
    .bind(&role)
    .bind(now)
    .execute(executor)
    .await?;

    Ok(UserRecord {
        id,
        email,
        first_name,
        last_name,
        password_hash,
        company_id,
        branch_id: None,
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

/// Retrieves a user's company ID by their user ID.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_user_company_id(pool: &PgPool, user_id: &str) -> Result<Option<String>> {
    #[derive(sqlx::FromRow)]
    struct CompanyIdRow {
        company_id: Option<String>,
    }

    let record = sqlx::query_as::<_, CompanyIdRow>(
        r"
        SELECT company_id
        FROM users
        WHERE id = $1
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(record.and_then(|r| r.company_id))
}

/// Retrieves a user by their email address.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_user_by_email(pool: &PgPool, email: &str) -> Result<Option<UserRecord>> {
    let user = sqlx::query_as::<_, UserRecord>(
        &format!("{USER_SELECT_COLUMNS}\n        WHERE users.email = $1 AND users.deleted_at IS NULL\n        "),
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

/// Retrieves a user by their ID.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_user_by_id(pool: &PgPool, id: &str) -> Result<Option<UserRecord>> {
    let user = sqlx::query_as::<_, UserRecord>(
        &format!("{USER_SELECT_COLUMNS}\n        WHERE users.id = $1 AND users.deleted_at IS NULL\n        "),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    if let Some(u) = &user {
        tracing::debug!(
            "DB: Found user {} with company_id: {:?}, branch_id: {:?}",
            u.email,
            u.company_id,
            u.branch_id
        );
    } else {
        tracing::debug!("DB: User {} not found", id);
    }

    Ok(user)
}

/// Retrieves a user by OAuth provider and subject.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_user_by_oauth(
    pool: &PgPool,
    provider: &str,
    subject: &str,
) -> Result<Option<UserRecord>> {
    let user = sqlx::query_as::<_, UserRecord>(
        &format!("{USER_SELECT_COLUMNS}\n        WHERE users.oauth_provider = $1 AND users.oauth_subject = $2 AND users.deleted_at IS NULL\n        "),
    )
    .bind(provider)
    .bind(subject)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}

/// Creates a new user with OAuth authentication.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_oauth_user<'a, E>(
    executor: E,
    email: String,
    first_name: String,
    last_name: String,
    oauth_provider: String,
    oauth_subject: String,
    oauth_picture: Option<String>,
    company_id: Option<String>,
    role: UserRole,
) -> Result<UserRecord>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO users (id, email, first_name, last_name, password_hash, company_id, role, created_at, oauth_provider, oauth_subject, oauth_picture)
        VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8, $9, $10)
        ",
    )
    .bind(&id)
    .bind(&email)
    .bind(&first_name)
    .bind(&last_name)
    .bind(&company_id)
    .bind(&role)
    .bind(now)
    .bind(&oauth_provider)
    .bind(&oauth_subject)
    .bind(&oauth_picture)
    .execute(executor)
    .await?;

    Ok(UserRecord {
        id,
        email,
        first_name,
        last_name,
        password_hash: None,
        company_id,
        branch_id: None,
        company_name: None,
        role,
        created_at: now,
        deleted_at: None,
        profile_picture_id: None,
        oauth_provider: Some(oauth_provider),
        oauth_subject: Some(oauth_subject),
        oauth_picture,
        company_deleted_at: None,
    })
}

/// Links an OAuth account to an existing user.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn link_oauth_to_user(
    pool: &PgPool,
    user_id: &str,
    oauth_provider: String,
    oauth_subject: String,
    oauth_picture: Option<String>,
) -> Result<()> {
    sqlx::query(
        r"
        UPDATE users
        SET oauth_provider = $1, oauth_subject = $2, oauth_picture = $3
        WHERE id = $4
        ",
    )
    .bind(&oauth_provider)
    .bind(&oauth_subject)
    .bind(&oauth_picture)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Unlinks OAuth authentication from a user.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn unlink_oauth_from_user(pool: &PgPool, user_id: &str) -> Result<()> {
    let query = sqlx::query(
        r"
        UPDATE users
        SET oauth_provider = NULL, oauth_subject = NULL, oauth_picture = NULL
        WHERE id = $1 AND oauth_provider IS NOT NULL AND oauth_subject IS NOT NULL
        ",
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    if query.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Oauth account not found for user"));
    }
    Ok(())
}

/// Soft deletes a user by their email address.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn delete_user_by_email(pool: &PgPool, email: &str) -> Result<()> {
    sqlx::query(
        r"
        UPDATE users
        SET deleted_at = $1
        WHERE email = $2 AND deleted_at IS NULL
        ",
    )
    .bind(chrono::Utc::now())
    .bind(email)
    .execute(pool)
    .await?;

    Ok(())
}

/// Soft deletes all users belonging to a company.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn soft_delete_users_by_company_id(pool: &PgPool, company_id: &str) -> Result<()> {
    sqlx::query(
        r"
        UPDATE users
        SET deleted_at = $1
        WHERE company_id = $2 AND deleted_at IS NULL
        ",
    )
    .bind(chrono::Utc::now())
    .bind(company_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Retrieves all users belonging to a specific company.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_users_by_company_id(pool: &PgPool, company_id: &str) -> Result<Vec<UserRecord>> {
    let users = sqlx::query_as::<_, UserRecord>(
        &format!("{USER_SELECT_COLUMNS}\n        WHERE users.company_id = $1 AND users.deleted_at IS NULL\n        ")
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

/// Retrieves all users belonging to a specific company, including soft-deleted ones.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_all_users_by_company_id(
    pool: &PgPool,
    company_id: &str,
) -> Result<Vec<UserRecord>> {
    let users = sqlx::query_as::<_, UserRecord>(
        &format!("{USER_SELECT_COLUMNS}\n        WHERE users.company_id = $1\n        ")
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

/// Retrieves all members of the same company as the requesting user.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_company_members_for_user(pool: &PgPool, user_id: &str) -> Result<Vec<UserRecord>> {
    tracing::info!("DB: Fetching members for user_id: {}", user_id);
    let users = sqlx::query_as::<_, UserRecord>(
        r"
        SELECT target_user.id, target_user.email, target_user.first_name, target_user.last_name, 
               target_user.password_hash, target_user.company_id, target_user.branch_id, target_user.role, target_user.created_at, target_user.deleted_at, 
               companies.name as company_name, companies.deleted_at as company_deleted_at,
               target_user.oauth_provider, target_user.oauth_subject, target_user.oauth_picture, target_user.profile_picture_id
        FROM users as request_user
        JOIN users as target_user ON request_user.company_id = target_user.company_id
        LEFT JOIN companies ON target_user.company_id = companies.id
        WHERE request_user.id = $1 AND target_user.deleted_at IS NULL
        ",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    tracing::info!("DB: Found {} members", users.len());
    for u in &users {
        tracing::info!(
            "DB: Member email={}, company_id={:?}, branch_id={:?}",
            u.email,
            u.company_id,
            u.branch_id
        );
    }

    Ok(users)
}

/// Updates a user's profile information (name only).
///
/// # Errors
/// Returns an error if database update fails or user not found.
pub async fn update_user_profile(
    pool: &PgPool,
    user_id: &str,
    first_name: String,
    last_name: String,
) -> Result<UserRecord> {
    let user = sqlx::query_as::<_, UserRecord>(
        r"
        WITH updated AS (
            UPDATE users SET first_name = $1, last_name = $2 WHERE id = $3 RETURNING *
        )
        SELECT updated.id, updated.email, updated.first_name, updated.last_name,
               updated.password_hash, updated.company_id, updated.branch_id, updated.role, updated.created_at, updated.deleted_at,
               companies.name as company_name, companies.deleted_at as company_deleted_at,
               updated.oauth_provider, updated.oauth_subject, updated.oauth_picture, updated.profile_picture_id
        FROM updated
        LEFT JOIN companies ON updated.company_id = companies.id
        ",
    )
    .bind(&first_name)
    .bind(&last_name)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

pub async fn update_user_profile_picture_id(
    pool: &PgPool,
    user_id: &str,
    picture_id: Option<&str>,
) -> Result<UserRecord> {
    let user = sqlx::query_as::<_, UserRecord>(
        r"
        WITH updated AS (
            UPDATE users SET profile_picture_id = $1 WHERE id = $2 RETURNING *
        )
        SELECT updated.id, updated.email, updated.first_name, updated.last_name,
               updated.password_hash, updated.company_id, updated.branch_id, updated.role, updated.created_at, updated.deleted_at,
               companies.name as company_name, companies.deleted_at as company_deleted_at,
               updated.oauth_provider, updated.oauth_subject, updated.oauth_picture, updated.profile_picture_id
        FROM updated
        LEFT JOIN companies ON updated.company_id = companies.id
        ",
    )
    .bind(picture_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Updates a user's profile information including their role and branch.
///
/// # Errors
/// Returns an error if database update fails or user not found.
pub async fn update_user_profile_full(
    pool: &PgPool,
    user_id: &str,
    first_name: String,
    last_name: String,
    role: UserRole,
    branch_id: Option<String>,
    profile_picture_id: Option<String>,
) -> Result<UserRecord> {
    let user = sqlx::query_as::<_, UserRecord>(
        r"
        WITH updated AS (
            UPDATE users SET first_name = $1, last_name = $2, role = $3, branch_id = $4, profile_picture_id = $5 WHERE id = $6 RETURNING *
        )
        SELECT updated.id, updated.email, updated.first_name, updated.last_name,
               updated.password_hash, updated.company_id, updated.branch_id, updated.role, updated.created_at, updated.deleted_at,
               companies.name as company_name, companies.deleted_at as company_deleted_at,
               updated.oauth_provider, updated.oauth_subject, updated.oauth_picture, updated.profile_picture_id
        FROM updated
        LEFT JOIN companies ON updated.company_id = companies.id
        ",
    )
    .bind(&first_name)
    .bind(&last_name)
    .bind(&role)
    .bind(&branch_id)
    .bind(&profile_picture_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Updates a user's password hash.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn update_user_password(
    pool: &PgPool,
    user_id: &str,
    password_hash: String,
) -> Result<()> {
    sqlx::query(
        r"
        UPDATE users
        SET password_hash = $1
        WHERE id = $2
        ",
    )
    .bind(&password_hash)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Updates the branch association for a user.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn update_user_branch(
    pool: &PgPool,
    user_id: &str,
    branch_id: Option<String>,
) -> Result<()> {
    sqlx::query(
        r"
        UPDATE users
        SET branch_id = $1
        WHERE id = $2
        ",
    )
    .bind(branch_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}
