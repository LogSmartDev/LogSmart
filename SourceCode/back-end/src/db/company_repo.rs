use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;
use super::COMPANY_RETURNING_COLUMNS;

/// Creates a new company in the database.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_company<'a, E>(executor: E, name: String, address: String) -> Result<Company>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO companies (id, name, address, created_at)
        VALUES ($1, $2, $3, $4)
        ",
    )
    .bind(&id)
    .bind(&name)
    .bind(&address)
    .bind(now)
    .execute(executor)
    .await?;

    Ok(Company {
        id,
        name,
        address,
        created_at: now,
        ..Company::new()
    })
}

/// Updates a given company's logo
///
/// # Errors
/// Returns an error if database query fails
pub async fn update_company_logo_id(
    pool: &PgPool,
    company_id: &str,
    company_logo_id: Option<&str>,
) -> Result<Company> {
    sqlx::query_as(
        &format!("UPDATE companies\n        SET logo_id = $1\n        WHERE id = $2\n        {COMPANY_RETURNING_COLUMNS}\n        "),
    )
    .bind(company_logo_id)
    .bind(company_id)
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update company logo: {e}"))
}

/// Retrieves a company by its ID.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_company_by_id(pool: &PgPool, id: &str) -> Result<Option<Company>> {
    let company = sqlx::query_as::<_, Company>(
        r"
        SELECT id, name, address, created_at, logo_id, data_exported_at, deleted_at, deletion_requested_at, deletion_token, deletion_requested_by_email
        FROM companies
        WHERE id = $1 AND deleted_at IS NULL
        ",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(company)
}

/// Updates a company's name and address.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn update_company(
    pool: &PgPool,
    company_id: &str,
    name: &str,
    address: &str,
) -> Result<Company> {
    sqlx::query_as(
        &format!("UPDATE companies\n        SET name = $1, address = $2\n        WHERE id = $3\n        {COMPANY_RETURNING_COLUMNS}\n        "),
    )
    .bind(name)
    .bind(address)
    .bind(company_id)
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to update company: {e}"))
}

/// Marks company data as exported.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn mark_company_data_exported(pool: &PgPool, company_id: &str) -> Result<Company> {
    sqlx::query_as(
        &format!("UPDATE companies\n        SET data_exported_at = NOW()\n        WHERE id = $1\n        {COMPANY_RETURNING_COLUMNS}\n        "),
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to mark company data as exported: {e}"))
}

/// Requests company deletion with a confirmation token.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn request_company_deletion(
    pool: &PgPool,
    company_id: &str,
    requester_email: &str,
) -> Result<Company> {
    let token = Uuid::new_v4().to_string();
    sqlx::query_as(
        &format!("UPDATE companies\n        SET deletion_requested_at = NOW(), deletion_token = $1, deletion_requested_by_email = $2\n        WHERE id = $3\n        {COMPANY_RETURNING_COLUMNS}\n        "),
    )
    .bind(&token)
    .bind(requester_email)
    .bind(company_id)
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to request company deletion: {e}"))
}

/// Confirms company deletion with token.
///
/// # Errors
/// Returns an error if database query fails or token is invalid/expired.
pub async fn confirm_company_deletion(
    pool: &PgPool,
    company_id: &str,
    token: &str,
) -> Result<Option<Company>> {
    let company = sqlx::query_as(
        &format!("UPDATE companies\n        SET deleted_at = NOW(), deletion_token = NULL, deletion_requested_at = NULL\n        WHERE id = $1 AND deletion_token = $2 AND deletion_requested_at IS NOT NULL AND deletion_requested_at > NOW() - INTERVAL '6 hours'\n        {COMPANY_RETURNING_COLUMNS}\n        "),
    )
    .bind(company_id)
    .bind(token)
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to confirm company deletion: {e}"))?;

    Ok(company)
}
