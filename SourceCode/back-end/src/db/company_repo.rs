use crate::error::DbError;
use sqlx::PgPool;
use uuid::Uuid;

use super::COMPANY_RETURNING_COLUMNS;
use super::types::*;

/// Creates a new company in the database.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_company<'a, E>(
    executor: E,
    name: String,
    address: String,
) -> Result<Company, DbError>
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
) -> Result<Company, DbError> {
    let mut query_builder = sqlx::QueryBuilder::new("UPDATE companies\nSET logo_id = ");
    query_builder.push_bind(company_logo_id);
    query_builder.push("\nWHERE id = ");
    query_builder.push_bind(company_id);
    query_builder.push("\n");
    query_builder.push(COMPANY_RETURNING_COLUMNS);
    query_builder.push("\n");
    
    query_builder
        .build_query_as::<Company>()
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::Internal(format!("Failed to update company logo: {}", e)))
}

/// Retrieves a company by its ID.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_company_by_id(pool: &PgPool, id: &str) -> Result<Option<Company>, DbError> {
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
) -> Result<Company, DbError> {
    let mut query_builder = sqlx::QueryBuilder::new("UPDATE companies\nSET name = ");
    query_builder.push_bind(name);
    query_builder.push(", address = ");
    query_builder.push_bind(address);
    query_builder.push("\nWHERE id = ");
    query_builder.push_bind(company_id);
    query_builder.push("\n");
    query_builder.push(COMPANY_RETURNING_COLUMNS);
    query_builder.push("\n");
    
    query_builder
        .build_query_as::<Company>()
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::Internal(format!("Failed to update company: {}", e)))
}

/// Marks company data as exported.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn mark_company_data_exported(
    pool: &PgPool,
    company_id: &str,
) -> Result<Company, DbError> {
    let mut query_builder = sqlx::QueryBuilder::new("UPDATE companies\nSET data_exported_at = NOW()\nWHERE id = ");
    query_builder.push_bind(company_id);
    query_builder.push("\n");
    query_builder.push(COMPANY_RETURNING_COLUMNS);
    query_builder.push("\n");
    
    query_builder
        .build_query_as::<Company>()
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::Internal(format!("Failed to mark company data as exported: {}", e)))
}

/// Requests company deletion with a confirmation token.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn request_company_deletion(
    pool: &PgPool,
    company_id: &str,
    requester_email: &str,
) -> Result<Company, DbError> {
    let token = Uuid::new_v4().to_string();
    let mut query_builder = sqlx::QueryBuilder::new("UPDATE companies\nSET deletion_requested_at = NOW(), deletion_token = ");
    query_builder.push_bind(&token);
    query_builder.push(", deletion_requested_by_email = ");
    query_builder.push_bind(requester_email);
    query_builder.push("\nWHERE id = ");
    query_builder.push_bind(company_id);
    query_builder.push("\n");
    query_builder.push(COMPANY_RETURNING_COLUMNS);
    query_builder.push("\n");
    
    query_builder
        .build_query_as::<Company>()
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::Internal(format!("Failed to request company deletion: {}", e)))
}

/// Confirms company deletion with token.
///
/// # Errors
/// Returns an error if database query fails or token is invalid/expired.
pub async fn confirm_company_deletion(
    pool: &PgPool,
    company_id: &str,
    token: &str,
) -> Result<Option<Company>, DbError> {
    let mut query_builder = sqlx::QueryBuilder::new("UPDATE companies\nSET deleted_at = NOW(), deletion_token = NULL, deletion_requested_at = NULL\nWHERE id = ");
    query_builder.push_bind(company_id);
    query_builder.push(" AND deletion_token = ");
    query_builder.push_bind(token);
    query_builder.push(" AND deletion_requested_at IS NOT NULL AND deletion_requested_at > NOW() - INTERVAL '6 hours'\n");
    query_builder.push(COMPANY_RETURNING_COLUMNS);
    query_builder.push("\n");
    
    let company = query_builder
        .build_query_as::<Company>()
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Internal(format!("Failed to confirm company deletion: {}", e)))?;

    Ok(company)
}
