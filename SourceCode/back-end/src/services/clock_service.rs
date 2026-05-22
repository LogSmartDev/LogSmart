use crate::db;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

pub struct ClockService;

const MAX_CLOCK_DURATION_HOURS: i64 = 24;

fn capped_clock_out_time(clock_in: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    let max_clock_out = clock_in + Duration::hours(MAX_CLOCK_DURATION_HOURS);
    if now > max_clock_out {
        max_clock_out
    } else {
        now
    }
}

impl ClockService {
    /// Clocks in the user. Returns error if already clocked in.
    ///
    /// # Errors
    /// Returns an error if the user is already clocked in or DB operations fail.
    pub async fn clock_in(
        pool: &PgPool,
        user_id: &str,
        company_id: &str,
    ) -> Result<db::ClockEvent, crate::error::AppError> {
        // Check if user already has an open clock-in
        let current = db::get_clock_status(pool, user_id).await.map_err(|e| {
            tracing::error!("Database error checking clock status: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?;

        if let Some(ref event) = current
            && event.is_clocked_in()
        {
            return Err(crate::error::AppError::Conflict("You are already clocked in".to_string()));
        }

        let event = db::clock_in(pool, user_id, company_id).await.map_err(|e| {
            tracing::error!("Database error clocking in: {:?}", e);
            crate::error::AppError::Internal("Failed to clock in".to_string())
        })?;

        Ok(event)
    }

    /// Clocks out the user. Returns error if not currently clocked in.
    ///
    /// # Errors
    /// Returns an error if the user is not clocked in or DB operations fail.
    pub async fn clock_out(
        pool: &PgPool,
        user_id: &str,
    ) -> Result<db::ClockEvent, crate::error::AppError> {
        let current = db::get_clock_status(pool, user_id).await.map_err(|e| {
            tracing::error!("Database error checking clock status: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?;

        let Some(current_event) = current else {
            return Err(crate::error::AppError::BadRequest("You are not currently clocked in".to_string()));
        };

        if !current_event.is_clocked_in() {
            return Err(crate::error::AppError::BadRequest("You are not currently clocked in".to_string()));
        }

        let now = Utc::now();
        let clock_out_at = capped_clock_out_time(current_event.clock_in, now);

        let event = db::clock_out_at(pool, user_id, clock_out_at)
            .await
            .map_err(|e| {
                tracing::error!("Database error clocking out: {:?}", e);
                crate::error::AppError::Internal("Failed to clock out".to_string())
            })?;

        event.ok_or(crate::error::AppError::BadRequest("You are not currently clocked in".to_string()))
    }

    /// Gets the current clock status and recent events for a user.
    ///
    /// # Errors
    /// Returns an error if DB operations fail.
    pub async fn get_status(
        pool: &PgPool,
        user_id: &str,
    ) -> Result<(Option<db::ClockEvent>, Vec<db::ClockEvent>), crate::error::AppError>
    {
        let current = db::get_clock_status(pool, user_id).await.map_err(|e| {
            tracing::error!("Database error fetching clock status: {:?}", e);
            crate::error::AppError::Internal("Database error".to_string())
        })?;

        let recent = db::get_recent_clock_events(pool, user_id, 5)
            .await
            .map_err(|e| {
                tracing::error!("Database error fetching recent clock events: {:?}", e);
                crate::error::AppError::Internal("Database error".to_string())
            })?;

        Ok((current, recent))
    }

    pub async fn get_company_clock_events(
        pool: &PgPool,
        company_id: &str,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        branch_id: Option<String>,
        limit: Option<i64>,
        cursor: Option<String>,
    ) -> Result<(Vec<db::CompanyClockEventRow>, Option<String>), crate::error::AppError>
    {
        let (events, next_cursor) =
            db::get_company_clock_events(pool, company_id, from, to, branch_id, limit, cursor)
                .await
                .map_err(|e| {
                    tracing::error!("Database error fetching company clock events: {:?}", e);
                    crate::error::AppError::Internal("Database error".to_string())
                })?;
        Ok((events, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_to_exactly_24_hours_when_over_limit() {
        let clock_in = Utc::now() - Duration::hours(24);
        let now = clock_in + Duration::hours(26);
        let capped = capped_clock_out_time(clock_in, now);
        assert_eq!(capped, clock_in + Duration::hours(24));
    }

    #[test]
    fn keeps_current_time_when_within_limit() {
        let now = Utc::now();
        let clock_in = now - Duration::hours(23);
        let capped = capped_clock_out_time(clock_in, now);
        assert_eq!(capped, now);
    }
}
