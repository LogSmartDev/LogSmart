use std::fmt::Write;
use crate::error::DbError;

use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;
use super::SECURITY_LOG_SELECT_COLUMNS;
use super::{encode_cursor, decode_cursor};

/// Logs a security event to the database.
///
/// # Errors
/// Returns an error if database insert fails.
pub async fn log_security_event(
    pool: &PgPool,
    event_type: String,
    user_id: Option<String>,
    email: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    meta: SecurityLogMeta,
    details: Option<String>,
    success: bool,
) -> Result<SecurityLog, DbError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    sqlx::query(
        r"
        INSERT INTO security_logs (
            id, event_type, user_id, email, ip_address, user_agent,
            actor_role, company_id, target_user_id, target_email, request_path, request_method,
            details, success, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ",
    )
    .bind(&id)
    .bind(&event_type)
    .bind(&user_id)
    .bind(&email)
    .bind(&ip_address)
    .bind(&user_agent)
    .bind(&meta.actor_role)
    .bind(&meta.company_id)
    .bind(&meta.target_user_id)
    .bind(&meta.target_email)
    .bind(&meta.request_path)
    .bind(&meta.request_method)
    .bind(&details)
    .bind(success)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(SecurityLog {
        id,
        event_type,
        user_id,
        email,
        ip_address,
        user_agent,
        actor_role: meta.actor_role,
        company_id: meta.company_id,
        target_user_id: meta.target_user_id,
        target_email: meta.target_email,
        request_path: meta.request_path,
        request_method: meta.request_method,
        details,
        success,
        created_at: now,
    })
}

/// Retrieves security logs for a specific user.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_security_logs_by_user(
    pool: &PgPool,
    user_id: &str,
    limit: i64,
) -> Result<Vec<SecurityLog>, DbError> {
    let logs = sqlx::query_as::<_, SecurityLog>(
        &format!("{SECURITY_LOG_SELECT_COLUMNS}\n        WHERE user_id = $1\n        ORDER BY created_at DESC\n        LIMIT $2\n        "),
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(logs)
}

/// Retrieves recent security logs, optionally filtered by event type.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_recent_security_logs(
    pool: &PgPool,
    event_type: Option<String>,
    limit: i64,
) -> Result<Vec<SecurityLog>, DbError> {
    let logs = if let Some(evt) = event_type {
        sqlx::query_as::<_, SecurityLog>(
            &format!("{SECURITY_LOG_SELECT_COLUMNS}\n            WHERE event_type = $1\n            ORDER BY created_at DESC\n            LIMIT $2\n            "),
        )
        .bind(evt)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, SecurityLog>(
            &format!("{SECURITY_LOG_SELECT_COLUMNS}\n            ORDER BY created_at DESC\n            LIMIT $1\n            "),
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(logs)
}

/// Builds WHERE clause conditions for security log filters.
/// Appends conditions to `query_str` and updates `bind_count` with the number of binds added.
pub(super) fn build_security_log_filter_conditions(
    query_str: &mut String,
    bind_count: &mut i64,
    filters: &SecurityLogFilters,
) -> std::fmt::Result {
    if filters.event_type.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND event_type ILIKE ${bind_count}")?;
    }
    if filters.user_id.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND user_id ILIKE ${bind_count}")?;
    }
    if filters.email.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND email ILIKE ${bind_count}")?;
    }
    if filters.ip_address.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND ip_address ILIKE ${bind_count}")?;
    }
    if filters.user_agent.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND user_agent ILIKE ${bind_count}")?;
    }
    if filters.actor_role.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND actor_role ILIKE ${bind_count}")?;
    }
    if filters.company_id.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND company_id = ${bind_count}")?;
    }
    if filters.target_user_id.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND target_user_id ILIKE ${bind_count}")?;
    }
    if filters.target_email.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND target_email ILIKE ${bind_count}")?;
    }
    if filters.request_path.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND request_path ILIKE ${bind_count}")?;
    }
    if filters.request_method.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND request_method ILIKE ${bind_count}")?;
    }
    if filters.details.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND details ILIKE ${bind_count}")?;
    }
    if filters.success.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND success = ${bind_count}")?;
    }
    if filters.created_from.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND created_at >= ${bind_count}")?;
    }
    if filters.created_to.is_some() {
        *bind_count += 1;
        writeln!(query_str, "  AND created_at <= ${bind_count}")?;
    }
    Ok(())
}

/// Binds security log filter values to a query in the same order as `build_security_log_filter_conditions`.
pub(super) fn bind_security_log_filters<'q>(
    query: sqlx::query::QueryAs<'q, sqlx::Postgres, SecurityLog, sqlx::postgres::PgArguments>,
    filters: &'q SecurityLogFilters,
) -> sqlx::query::QueryAs<'q, sqlx::Postgres, SecurityLog, sqlx::postgres::PgArguments> {
    let mut q = query;
    if let Some(value) = filters.event_type.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.user_id.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.email.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.ip_address.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.user_agent.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.actor_role.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.company_id.as_ref() {
        q = q.bind(value.trim());
    }
    if let Some(value) = filters.target_user_id.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.target_email.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.request_path.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.request_method.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.details.as_ref() {
        q = q.bind(format!("%{}%", value.trim()));
    }
    if let Some(value) = filters.success {
        q = q.bind(value);
    }
    if let Some(value) = filters.created_from {
        q = q.bind(value);
    }
    if let Some(value) = filters.created_to {
        q = q.bind(value);
    }
    q
}

/// Retrieves paginated security logs with optional filters using keyset pagination.
///
/// Cursor format: base64("<created_at_rfc3339>|<id>")
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_security_logs_page(
    pool: &PgPool,
    filters: &SecurityLogFilters,
    limit: i64,
    cursor: Option<(chrono::DateTime<chrono::Utc>, String)>,
) -> Result<SecurityLogsPage, DbError> {
    let safe_limit = limit.clamp(1, 100);
    let fetch_limit = safe_limit + 1;

    let mut query_str = format!("{SECURITY_LOG_SELECT_COLUMNS}\n        WHERE 1=1\n        ");
    let mut bind_count: i64 = 0;

    build_security_log_filter_conditions(&mut query_str, &mut bind_count, filters)?;

    let cursor_data = cursor.as_ref().map(|(created_at, id)| (created_at, id.clone()));
    if let Some((_cursor_created_at, _cursor_id)) = &cursor_data {
        bind_count += 1;
        let cursor_created_at_bind = bind_count;
        bind_count += 1;
        let cursor_id_bind = bind_count;
        writeln!(
            query_str,
            "  AND (created_at < ${cursor_created_at_bind} OR (created_at = ${cursor_created_at_bind} AND id < ${cursor_id_bind}))"
        )?;
    }

    bind_count += 1;
    writeln!(
        query_str,
        "ORDER BY created_at DESC, id DESC\nLIMIT ${bind_count}"
    )?;

    let mut query = bind_security_log_filters(sqlx::query_as::<_, SecurityLog>(&query_str), filters);

    if let Some((cursor_created_at, cursor_id)) = cursor_data {
        query = query.bind(cursor_created_at).bind(cursor_id);
    }

    query = query.bind(fetch_limit);

    let mut rows = query.fetch_all(pool).await?;
    let has_more = rows.len() > safe_limit as usize;
    if has_more {
        rows.truncate(safe_limit as usize);
    }

    let next_cursor = if has_more {
        rows.last()
            .map(|row| format_security_logs_cursor(row.created_at, &row.id))
    } else {
        None
    };

    Ok(SecurityLogsPage {
        logs: rows,
        next_cursor,
    })
}

/// Retrieves security logs for CSV export with optional filters.
///
/// # Errors
/// Returns an error if database query fails.
pub async fn get_security_logs_for_export(
    pool: &PgPool,
    filters: &SecurityLogFilters,
    limit: i64,
) -> Result<Vec<SecurityLog>, DbError> {
    let safe_limit = limit.clamp(1, 10_000);

    let mut query_str = format!("{SECURITY_LOG_SELECT_COLUMNS}\n        WHERE 1=1\n        ");
    let mut bind_count: i64 = 0;

    build_security_log_filter_conditions(&mut query_str, &mut bind_count, filters)?;

    bind_count += 1;
    writeln!(
        query_str,
        "ORDER BY created_at DESC, id DESC\nLIMIT ${bind_count}"
    )?;

    let query = bind_security_log_filters(sqlx::query_as::<_, SecurityLog>(&query_str), filters);
    let rows = query.bind(safe_limit).fetch_all(pool).await?;
    Ok(rows)
}

#[must_use]
pub fn format_security_logs_cursor(created_at: chrono::DateTime<chrono::Utc>, id: &str) -> String {
    encode_cursor(created_at, id)
}

pub fn parse_security_logs_cursor(cursor: &str) -> Result<(chrono::DateTime<chrono::Utc>, String), DbError> {
    decode_cursor(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_logs_cursor_round_trip() {
        let created_at = chrono::Utc::now();
        let id = "security-log-id-123";

        let cursor = format_security_logs_cursor(created_at, id);
        let parsed = parse_security_logs_cursor(&cursor).expect("cursor should parse");

        assert_eq!(parsed.1, id);
        assert_eq!(parsed.0.timestamp_millis(), created_at.timestamp_millis());
    }

    #[test]
    fn test_security_logs_cursor_invalid_value() {
        let result = parse_security_logs_cursor("not-a-valid-cursor");
        assert!(result.is_err(), "invalid cursor must return error");
    }
}
