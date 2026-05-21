use std::fmt::Write;

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;
use super::{CLOCK_EVENT_SELECT_COLUMNS, CLOCK_EVENT_RETURNING};
use super::{encode_cursor, decode_cursor};

/// Creates a new clock-in event for a user.
///
/// # Errors
/// Returns an error if the database insert fails.
pub async fn clock_in(pool: &PgPool, user_id: &str, company_id: &str) -> Result<ClockEvent> {
    let id = Uuid::new_v4().to_string();
    let event = sqlx::query_as::<_, ClockEvent>(
        &format!("INSERT INTO clock_events (id, user_id, company_id, clock_in)\n        VALUES ($1, $2, $3, CURRENT_TIMESTAMP)\n        {CLOCK_EVENT_RETURNING}\n        "),
    )
    .bind(&id)
    .bind(user_id)
    .bind(company_id)
    .fetch_one(pool)
    .await?;

    Ok(event)
}

/// Clocks out a user by updating the most recent open clock-in event.
///
/// # Errors
/// Returns an error if the database update fails.
pub async fn clock_out(pool: &PgPool, user_id: &str) -> Result<Option<ClockEvent>> {
    let event = sqlx::query_as::<_, ClockEvent>(
        &format!("UPDATE clock_events\n        SET clock_out = CURRENT_TIMESTAMP\n        WHERE id = (\n            SELECT id FROM clock_events\n            WHERE user_id = $1 AND clock_out IS NULL\n            ORDER BY clock_in DESC\n            LIMIT 1\n        )\n        {CLOCK_EVENT_RETURNING}\n        "),
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(event)
}

/// Clocks out a user at a specific timestamp by updating the most recent open clock-in event.
/// This is used to cap long-running sessions (e.g., max 24 hours) at the service layer.
///
/// # Errors
/// Returns an error if the database update fails.
pub async fn clock_out_at(
    pool: &PgPool,
    user_id: &str,
    clock_out_at: chrono::DateTime<chrono::Utc>,
) -> Result<Option<ClockEvent>> {
    let event = sqlx::query_as::<_, ClockEvent>(
        &format!("UPDATE clock_events\n        SET clock_out = $2\n        WHERE id = (\n            SELECT id FROM clock_events\n            WHERE user_id = $1 AND clock_out IS NULL\n            ORDER BY clock_in DESC\n            LIMIT 1\n        )\n        {CLOCK_EVENT_RETURNING}\n        "),
    )
    .bind(user_id)
    .bind(clock_out_at)
    .fetch_optional(pool)
    .await?;

    Ok(event)
}

/// Gets the current clock status for a user (latest event).
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_clock_status(pool: &PgPool, user_id: &str) -> Result<Option<ClockEvent>> {
    let event = sqlx::query_as::<_, ClockEvent>(
        &format!("{CLOCK_EVENT_SELECT_COLUMNS}\n        WHERE user_id = $1\n        ORDER BY created_at DESC\n        LIMIT 1\n        "),
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(event)
}

/// Gets the last N clock events for a user.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_recent_clock_events(
    pool: &PgPool,
    user_id: &str,
    limit: i64,
) -> Result<Vec<ClockEvent>> {
    let events = sqlx::query_as::<_, ClockEvent>(
        &format!("{CLOCK_EVENT_SELECT_COLUMNS}\n        WHERE user_id = $1\n        ORDER BY created_at DESC\n        LIMIT $2\n        "),
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(events)
}

/// Gets all clock events for a company, joined with user name/email.
/// Optionally filtered by date range and branch.
/// Uses cursor-based pagination.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_company_clock_events(
    pool: &PgPool,
    company_id: &str,
    from: Option<chrono::DateTime<chrono::Utc>>,
    to: Option<chrono::DateTime<chrono::Utc>>,
    branch_id: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
) -> Result<(Vec<CompanyClockEventRow>, Option<String>)> {
    let limit = limit.unwrap_or(25).max(1).min(100);
    let cursor = cursor.and_then(|c| parse_clock_events_cursor(&c).ok());

    let branch_id = branch_id.filter(|s| !s.is_empty());

    let mut query_str = String::from(
        r"
        SELECT ce.id, ce.user_id, ce.company_id, ce.clock_in, ce.clock_out,
               ce.created_at,
               u.first_name, u.last_name, u.email
        FROM clock_events ce
        JOIN users u ON u.id = ce.user_id
        WHERE ce.company_id = $1
        ",
    );

    let mut bind_count = 1;

    if cursor.is_some() {
        bind_count += 1;
        let cursor_param_1 = bind_count;
        bind_count += 1;
        let cursor_param_2 = bind_count;
        writeln!(
            query_str,
            "  AND (ce.clock_in, ce.id) < (${cursor_param_1}, ${cursor_param_2})"
        )?;
    }

    if branch_id.is_some() {
        bind_count += 1;
        writeln!(query_str, "  AND u.branch_id = ${bind_count}")?;
    }

    if from.is_some() {
        bind_count += 1;
        writeln!(query_str, "  AND ce.clock_in >= ${bind_count}")?;
    }

    if to.is_some() {
        bind_count += 1;
        writeln!(query_str, "  AND ce.clock_in <= ${bind_count}")?;
    }

    writeln!(query_str, "ORDER BY ce.clock_in DESC, ce.id DESC")?;
    writeln!(query_str, "LIMIT {}", limit + 1)?;

    tracing::debug!("Clock events query: {}", query_str);
    tracing::debug!("Branch ID filter: {:?}", branch_id);

    let mut query = sqlx::query_as::<_, CompanyClockEventRow>(&query_str).bind(company_id);

    if let Some((cursor_time, cursor_id)) = &cursor {
        query = query.bind(cursor_time).bind(cursor_id);
    }

    if let Some(bid) = branch_id {
        query = query.bind(bid);
    }

    if let Some(f) = from {
        query = query.bind(f);
    }

    if let Some(t) = to {
        query = query.bind(t);
    }

    let mut events = query.fetch_all(pool).await?;

    let next_cursor = if events.len() > limit as usize {
        let last_idx = (limit as usize) - 1;
        let last_clock_in = events[last_idx].clock_in;
        let last_id = events[last_idx].id.clone();
        events.truncate(limit as usize);
        Some(create_clock_events_cursor(last_clock_in, &last_id))
    } else {
        None
    };

    Ok((events, next_cursor))
}

pub fn create_clock_events_cursor(created_at: chrono::DateTime<chrono::Utc>, id: &str) -> String {
    encode_cursor(created_at, id)
}

pub fn parse_clock_events_cursor(cursor: &str) -> Result<(chrono::DateTime<chrono::Utc>, String)> {
    decode_cursor(cursor)
}
