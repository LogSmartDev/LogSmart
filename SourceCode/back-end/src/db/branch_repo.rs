use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;
use super::BRANCH_SELECT_COLUMNS;

/// Creates a new branch for a company.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_branch<'a, E>(
    executor: E,
    company_id: String,
    name: String,
    address: String,
) -> Result<Branch>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO branches (id, company_id, name, address, created_at)
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(&id)
    .bind(&company_id)
    .bind(&name)
    .bind(&address)
    .bind(now)
    .execute(executor)
    .await?;

    Ok(Branch {
        id,
        company_id,
        name,
        address,
        created_at: now,
    })
}

/// Retrieves all branches for a company with their deletion status.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_branches_by_company_id_with_deletion_status(
    pool: &PgPool,
    company_id: &str,
) -> Result<Vec<BranchWithDeletionStatus>> {
    #[derive(sqlx::FromRow)]
    struct BranchDeletionRow {
        id: String,
        company_id: String,
        name: String,
        address: String,
        created_at: chrono::DateTime<chrono::Utc>,
        has_pending: Option<bool>,
        requested_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows: Vec<BranchDeletionRow> = sqlx::query_as(
        r"
        SELECT 
            b.id, b.company_id, b.name, b.address, b.created_at,
            CASE WHEN COUNT(d.branch_id) > 0 THEN true ELSE false END as has_pending,
            MIN(d.created_at) as requested_at
        FROM branches b
        LEFT JOIN branch_deletion_tokens d ON b.id = d.branch_id AND d.used_at IS NULL AND d.expires_at > now()
        WHERE b.company_id = $1
        GROUP BY b.id, b.company_id, b.name, b.address, b.created_at
        ORDER BY b.name ASC
        ",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| BranchWithDeletionStatus {
            branch: Branch {
                id: row.id,
                company_id: row.company_id,
                name: row.name,
                address: row.address,
                created_at: row.created_at,
            },
            has_pending_deletion: row.has_pending.unwrap_or(false),
            deletion_requested_at: row.requested_at,
        })
        .collect())
}

/// Retrieves a branch by its ID.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_branch_by_id(pool: &PgPool, branch_id: &str) -> Result<Option<Branch>> {
    let branch = sqlx::query_as::<_, Branch>(
        &format!("{BRANCH_SELECT_COLUMNS}\n        WHERE id = $1\n        "),
    )
    .bind(branch_id)
    .fetch_optional(pool)
    .await?;

    Ok(branch)
}

/// Updates a branch's details.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn update_branch(
    pool: &PgPool,
    branch_id: &str,
    name: &str,
    address: &str,
) -> Result<Branch> {
    let updated_branch = sqlx::query_as::<_, Branch>(
        r"
        UPDATE branches
        SET name = $1, address = $2
        WHERE id = $3
        RETURNING id, company_id, name, address, created_at
        ",
    )
    .bind(name)
    .bind(address)
    .bind(branch_id)
    .fetch_one(pool)
    .await?;

    Ok(updated_branch)
}

/// Deletes a branch by its ID.
///
/// # Errors
/// Returns an error if database delete fails.
pub async fn delete_branch(pool: &PgPool, branch_id: &str) -> Result<()> {
    sqlx::query(
        r"
        DELETE FROM branches
        WHERE id = $1
        ",
    )
    .bind(branch_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Creates a branch deletion token.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn create_branch_deletion_token(
    pool: &PgPool,
    user_id: String,
    branch_id: String,
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO branch_deletion_tokens (id, user_id, branch_id, token, created_at, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ",
    )
    .bind(&id)
    .bind(&user_id)
    .bind(&branch_id)
    .bind(&token)
    .bind(now)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(id)
}

/// Retrieves a branch deletion token record by its token.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_branch_deletion_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<(String, String, String)>> {
    let result = sqlx::query_as::<_, (String, String, String)>(
        r"
        SELECT id, user_id, branch_id
        FROM branch_deletion_tokens
        WHERE token = $1 AND used_at IS NULL AND expires_at > now()
        ",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// Marks a branch deletion token as used.
///
/// # Errors
/// Returns an error if database update fails.
pub async fn mark_branch_deletion_token_used(pool: &PgPool, token_id: &str) -> Result<()> {
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        UPDATE branch_deletion_tokens
        SET used_at = $1
        WHERE id = $2
        ",
    )
    .bind(now)
    .bind(token_id)
    .execute(pool)
    .await?;

    Ok(())
}
