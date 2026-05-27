use anyhow::Result;
use chrono::{Datelike, Timelike};

use super::types::{
    AvailabilityStatus, Frequency, LogStatus, PeriodValidationError, Schedule, TemplateLayout,
};

/// Computes the start and end timestamps for the current period based on frequency.
/// Extracted from the three functions that duplicated this ~80-line match block.
pub(super) fn compute_period_bounds(
    frequency: &Frequency,
) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    let now = chrono::Utc::now();
    match frequency {
        Frequency::Daily => {
            let start = now
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap_or_else(|| panic!("Error finding day start: {now}"))
                .and_utc();
            let end = now
                .date_naive()
                .and_hms_opt(23, 59, 59)
                .unwrap_or_else(|| panic!("Error finding day end: {now}"))
                .and_utc();
            (start, end)
        }
        Frequency::Weekly => {
            let days_since_monday = now.weekday().num_days_from_sunday();
            let start = (now.date_naive() - chrono::Duration::days(i64::from(days_since_monday)))
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            let end = (start.date_naive() + chrono::Duration::days(6))
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc();
            (start, end)
        }
        Frequency::Monthly => {
            let start = now
                .date_naive()
                .with_day(1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            let next_month = if now.month() == 12 {
                now.date_naive()
                    .with_year(now.year() + 1)
                    .unwrap()
                    .with_month(1)
                    .unwrap()
            } else {
                now.date_naive().with_month(now.month() + 1).unwrap()
            };
            let end = (next_month - chrono::Duration::days(1))
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc();
            (start, end)
        }
        Frequency::Quarterly => {
            let quarter_start_month = ((now.date_naive().month() - 1) / 3) * 3 + 1;
            let start = now
                .date_naive()
                .with_month(quarter_start_month)
                .unwrap()
                .with_day(1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            let quarter_end_month = quarter_start_month + 2;
            let last_day = get_month_last_day(now.date_naive().year(), quarter_end_month);
            let end = now
                .date_naive()
                .with_month(quarter_end_month)
                .unwrap()
                .with_day(last_day)
                .unwrap()
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc();
            (start, end)
        }
        Frequency::Yearly => {
            let start = now
                .date_naive()
                .with_month(1)
                .unwrap()
                .with_day(1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            let end = now
                .date_naive()
                .with_month(12)
                .unwrap()
                .with_day(31)
                .unwrap()
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc();
            (start, end)
        }
    }
}

fn get_month_last_day(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

#[must_use]
pub fn parse_time_string(time_str: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

/// Validates that a year is in a reasonable range (>= 2000).
fn normalize_year(year: i32) -> Option<i32> {
    if year >= 2000 { Some(year) } else { None }
}

#[must_use]
pub fn compute_due_date_for_period(schedule: &Schedule, period: &str) -> Option<chrono::NaiveDate> {
    let parts: Vec<&str> = period.split('/').collect();

    match schedule.frequency {
        Frequency::Daily => parse_period_to_date(period),
        Frequency::Weekly => {
            if parts.len() != 3 || !parts[0].contains('-') {
                return None;
            }
            let week_parts: Vec<&str> = parts[0].split('-').collect();
            if week_parts.len() != 2 {
                return None;
            }
            let _start_day: u32 = week_parts[0].parse().ok()?;
            let end_day: u32 = week_parts[1].parse().ok()?;
            let month: u32 = parts[1].parse().ok()?;
            let year: i32 = parts[2].parse().ok()?;
            let week_end = chrono::NaiveDate::from_ymd_opt(normalize_year(year)?, month, end_day)?;
            let week_start = week_end - chrono::Duration::days(6);
            let target_day = u32::from(schedule.day_of_week.unwrap_or(0));
            let days_to_add = i64::from(target_day)
                .wrapping_sub(i64::from(week_start.weekday().num_days_from_sunday()));
            let days_to_add = if days_to_add < 0 {
                days_to_add + 7
            } else {
                days_to_add
            };
            Some(week_start + chrono::Duration::days(days_to_add))
        }
        Frequency::Monthly => {
            if parts.len() != 2 {
                return None;
            }
            let month: u32 = parts[0].parse().ok()?;
            let year: i32 = parts[1].parse().ok()?;
            let target_day = u32::from(schedule.day_of_month.unwrap_or(1));
            let last_day_of_month = chrono::NaiveDate::from_ymd_opt(
                if month == 12 { year + 1 } else { year },
                if month == 12 { 1 } else { month + 1 },
                1,
            )? - chrono::Duration::days(1);
            let day = target_day.min(last_day_of_month.day());
            chrono::NaiveDate::from_ymd_opt(year, month, day)
        }
        Frequency::Yearly => {
            if parts.len() != 1 {
                return None;
            }
            let year: i32 = parts[0].parse().ok()?;
            let month = u32::from(schedule.month_of_year.unwrap_or(1));
            let target_day = u32::from(schedule.day_of_month.unwrap_or(1));
            let last_day_of_month = chrono::NaiveDate::from_ymd_opt(
                if month == 12 { year + 1 } else { year },
                if month == 12 { 1 } else { month + 1 },
                1,
            )? - chrono::Duration::days(1);
            let day = target_day.min(last_day_of_month.day());
            chrono::NaiveDate::from_ymd_opt(year, month, day)
        }
        Frequency::Quarterly => {
            let parts: Vec<&str> = period.split('/').collect();
            if parts.len() != 2 {
                return None;
            }
            let quarter: u32 = parts[0].parse().ok()?;
            let year: i32 = parts[1].parse().ok()?;
            let month = (quarter - 1) * 3 + 1;
            chrono::NaiveDate::from_ymd_opt(year, month, 1)
        }
    }
}

#[must_use]
pub fn get_availability_status_for_period(
    schedule: &Schedule,
    period: &str,
    current_datetime: chrono::DateTime<chrono::Utc>,
) -> AvailabilityStatus {
    let target_date = compute_due_date_for_period(schedule, period);
    if let Some(target) = target_date {
        let today = current_datetime.date_naive();

        if target < today {
            return AvailabilityStatus::Overdue;
        }
        if target > today {
            return AvailabilityStatus::NotAvailable;
        }

        let current_time = current_datetime.hour() * 60 + current_datetime.minute();

        match schedule.frequency {
            Frequency::Daily => {
                if let (Some(from_str), Some(due_str)) =
                    (&schedule.available_from_time, &schedule.due_at_time)
                    && let (Some((from_h, from_m)), Some((due_h, due_m))) =
                        (parse_time_string(from_str), parse_time_string(due_str))
                {
                    let from_mins = from_h * 60 + from_m;
                    let due_mins = due_h * 60 + due_m;

                    if current_time < from_mins {
                        return AvailabilityStatus::NotAvailable;
                    }
                    if current_time >= due_mins {
                        return AvailabilityStatus::Overdue;
                    }
                    return AvailabilityStatus::Available;
                }

                let default_from = 8 * 60;
                let default_due = 17 * 60;
                if current_time < default_from {
                    return AvailabilityStatus::NotAvailable;
                }
                if current_time >= default_due {
                    return AvailabilityStatus::Overdue;
                }
                AvailabilityStatus::Available
            }
            Frequency::Quarterly | Frequency::Weekly | Frequency::Monthly | Frequency::Yearly => {
                if current_time >= 23 * 60 + 59 {
                    return AvailabilityStatus::Overdue;
                }
                AvailabilityStatus::Available
            }
        }
    } else {
        AvailabilityStatus::Overdue
    }
}

#[must_use]
pub fn derive_log_status(
    stored_status: LogStatus,
    schedule: &Schedule,
    period: &str,
    current_datetime: chrono::DateTime<chrono::Utc>,
) -> (LogStatus, AvailabilityStatus) {
    let availability = get_availability_status_for_period(schedule, period, current_datetime);

    if stored_status == LogStatus::Overdue {
        return (stored_status, availability);
    }

    if availability == AvailabilityStatus::Overdue && stored_status != LogStatus::Submitted {
        return (LogStatus::Overdue, availability);
    }

    (stored_status, availability)
}

#[must_use]
pub fn parse_period_to_date(period: &str) -> Option<chrono::NaiveDate> {
    let parts: Vec<&str> = period.split('/').collect();

    match parts.len() {
        1 => {
            let year: i32 = parts[0].parse().ok()?;
            let year = normalize_year(year)?;
            chrono::NaiveDate::from_ymd_opt(year, 1, 1)
        }
        2 => {
            let month: u32 = parts[0].parse().ok()?;
            let year: i32 = parts[1].parse().ok()?;
            let year = normalize_year(year)?;
            chrono::NaiveDate::from_ymd_opt(year, month, 1)
        }
        3 => {
            if parts[0].contains('-') {
                let week_parts: Vec<&str> = parts[0].split('-').collect();
                if week_parts.len() != 2 {
                    return None;
                }
                let _start_day: u32 = week_parts[0].parse().ok()?;
                let end_day: u32 = week_parts[1].parse().ok()?;
                let month: u32 = parts[1].parse().ok()?;
                let year: i32 = parts[2].parse().ok()?;
                let year = normalize_year(year)?;
                chrono::NaiveDate::from_ymd_opt(year, month, end_day)
            } else {
                let day: u32 = parts[0].parse().ok()?;
                let month: u32 = parts[1].parse().ok()?;
                let year: i32 = parts[2].parse().ok()?;
                let year = normalize_year(year)?;
                chrono::NaiveDate::from_ymd_opt(year, month, day)
            }
        }
        _ => None,
    }
}

#[must_use]
pub fn validate_and_normalize_period(schedule: &Schedule, period: &str) -> Option<String> {
    let period = period.trim();
    match schedule.frequency {
        Frequency::Daily => {
            let parts: Vec<&str> = period.split('/').collect();
            if parts.len() != 3 || parts[0].contains('-') {
                return None;
            }
            let date = parse_period_to_date(period)?;
            Some(format_period_for_date(date))
        }
        Frequency::Weekly => {
            let parts: Vec<&str> = period.split('/').collect();
            if parts.len() != 3 || !parts[0].contains('-') {
                return None;
            }
            let date = parse_period_to_date(period)?;
            Some(format_period_for_weekly(date))
        }
        Frequency::Monthly => {
            let parts: Vec<&str> = period.split('/').collect();
            if parts.len() != 2 {
                return None;
            }
            let date = parse_period_to_date(period)?;
            Some(format_period_for_monthly(date))
        }
        Frequency::Quarterly => {
            let parts: Vec<&str> = period.split('/').collect();
            if parts.len() != 2 {
                return None;
            }
            let quarter: u32 = parts[0].parse().ok()?;
            let year: i32 = parts[1].parse().ok()?;
            if !(1..=4).contains(&quarter) || year < 2000 {
                return None;
            }
            Some(format!("{}/{:04}", quarter, year))
        }
        Frequency::Yearly => {
            let parts: Vec<&str> = period.split('/').collect();
            if parts.len() != 1 {
                return None;
            }
            let year: i32 = parts[0].parse().ok()?;
            if year < 2000 {
                return None;
            }
            Some(year.to_string())
        }
    }
}

pub fn validate_period_business_rules(
    schedule: &Schedule,
    period: &str,
    template_created_at: Option<chrono::DateTime<chrono::Utc>>,
    current_datetime: chrono::DateTime<chrono::Utc>,
) -> Result<String, PeriodValidationError> {
    let normalized = validate_and_normalize_period(schedule, period)
        .ok_or(PeriodValidationError::FormatInvalid)?;

    let due_date = compute_due_date_for_period(schedule, &normalized)
        .ok_or(PeriodValidationError::FormatInvalid)?;

    let today = current_datetime.date_naive();
    if due_date > today {
        return Err(PeriodValidationError::DueDateInFuture);
    }

    if let Frequency::Daily = schedule.frequency
        && let Some(days_of_week) = &schedule.days_of_week
    {
        let day_num = match due_date.weekday() {
            chrono::Weekday::Sun => 0,
            chrono::Weekday::Mon => 1,
            chrono::Weekday::Tue => 2,
            chrono::Weekday::Wed => 3,
            chrono::Weekday::Thu => 4,
            chrono::Weekday::Fri => 5,
            chrono::Weekday::Sat => 6,
        };
        if !days_of_week.contains(&day_num) {
            return Err(PeriodValidationError::WeekdayNotAllowed);
        }
    }

    if let Some(created_at) = template_created_at {
        let created_date = created_at.date_naive();
        if due_date < created_date {
            return Err(PeriodValidationError::BeforeTemplateCreation);
        }
    }

    Ok(normalized)
}

#[must_use]
pub fn is_form_due_today(schedule: &Schedule) -> bool {
    let today = chrono::Utc::now();
    let weekday = today.weekday();

    match schedule.frequency {
        Frequency::Daily => {
            if let Some(days) = &schedule.days_of_week {
                let day_num = match weekday {
                    chrono::Weekday::Sun => 0,
                    chrono::Weekday::Mon => 1,
                    chrono::Weekday::Tue => 2,
                    chrono::Weekday::Wed => 3,
                    chrono::Weekday::Thu => 4,
                    chrono::Weekday::Fri => 5,
                    chrono::Weekday::Sat => 6,
                };
                days.contains(&day_num)
            } else {
                true
            }
        }
        Frequency::Weekly => {
            if let Some(day) = schedule.day_of_week {
                let day_num = match weekday {
                    chrono::Weekday::Sun => 0,
                    chrono::Weekday::Mon => 1,
                    chrono::Weekday::Tue => 2,
                    chrono::Weekday::Wed => 3,
                    chrono::Weekday::Thu => 4,
                    chrono::Weekday::Fri => 5,
                    chrono::Weekday::Sat => 6,
                };
                day_num == day
            } else {
                false
            }
        }
        Frequency::Monthly => {
            if let Some(day) = schedule.day_of_month {
                today.day() >= u32::from(day)
            } else {
                false
            }
        }
        Frequency::Yearly => {
            if let Some(month) = schedule.month_of_year {
                if let Some(day) = schedule.day_of_month {
                    let target_month = u32::from(month);
                    let target_day = u32::from(day);

                    match today.month().cmp(&target_month) {
                        std::cmp::Ordering::Greater => true,
                        std::cmp::Ordering::Less => false,
                        std::cmp::Ordering::Equal => today.day() >= target_day,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }
        Frequency::Quarterly => {
            if let Some(month) = schedule.month_of_year {
                let current_quarter = ((today.month() - 1) / 3) + 1;
                let target_quarter = u32::from(month);
                if current_quarter == target_quarter
                    && let Some(day) = schedule.day_of_month
                {
                    return today.day() >= u32::from(day);
                }
            }
            false
        }
    }
}

#[must_use]
pub fn get_missed_periods(
    schedule: &Schedule,
    last_submitted_period: Option<&str>,
    created_at: Option<&str>,
) -> Vec<String> {
    let today = chrono::Utc::now().date_naive();
    let last_period = last_submitted_period.and_then(parse_period_to_date);

    let start_from = created_at.and_then(|c| {
        chrono::DateTime::parse_from_rfc3339(c)
            .ok()
            .map(|dt| dt.date_naive())
    });
    let created_date = start_from;

    let mut missed = Vec::new();

    match schedule.frequency {
        Frequency::Daily => {
            let days_of_week = schedule
                .days_of_week
                .clone()
                .unwrap_or_else(|| vec![0, 1, 2, 3, 4, 5, 6]);

            let start_date = last_period
                .map(|d| d + chrono::Duration::days(1))
                .unwrap_or_else(|| {
                    start_from.unwrap_or_else(|| {
                        tracing::warn!("Failed to determine start date for missed periods");
                        today
                    })
                });

            let mut current = start_date;
            while current <= today {
                let day_num = match current.weekday() {
                    chrono::Weekday::Sun => 0,
                    chrono::Weekday::Mon => 1,
                    chrono::Weekday::Tue => 2,
                    chrono::Weekday::Wed => 3,
                    chrono::Weekday::Thu => 4,
                    chrono::Weekday::Fri => 5,
                    chrono::Weekday::Sat => 6,
                };

                if days_of_week.contains(&day_num) {
                    missed.push(format_period_for_date(current));
                }
                current += chrono::Duration::days(1);
            }
        }
        Frequency::Weekly => {
            let target_day = u32::from(schedule.day_of_week.unwrap_or(0));

            let start_date = if let Some(last) = last_period {
                let day_after = last + chrono::Duration::days(1);
                let days_to_target = i64::from(target_day)
                    .wrapping_sub(i64::from(day_after.weekday().num_days_from_sunday()));
                let days_to_target = if days_to_target <= 0 {
                    days_to_target + 7
                } else {
                    days_to_target
                };
                day_after + chrono::Duration::days(days_to_target)
            } else if let Some(start) = start_from {
                let days_to_target = i64::from(target_day)
                    .wrapping_sub(i64::from(start.weekday().num_days_from_sunday()));
                let days_to_target = if days_to_target < 0 {
                    days_to_target + 7
                } else {
                    days_to_target
                };
                start + chrono::Duration::days(days_to_target)
            } else {
                let days_to_target = i64::from(target_day)
                    .wrapping_sub(i64::from(today.weekday().num_days_from_sunday()));
                let days_to_target = if days_to_target < 0 {
                    days_to_target + 7
                } else {
                    days_to_target
                };
                today + chrono::Duration::days(days_to_target)
            };

            let mut current = start_date;
            while current <= today {
                missed.push(format_period_for_weekly(current));
                current += chrono::Duration::weeks(1);
            }
        }
        Frequency::Monthly => {
            let target_day = schedule.day_of_month.unwrap_or(1);

            let start_date = last_period
                .map(|d| {
                    if d.month() == 12 {
                        chrono::NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).unwrap_or_else(|| {
                            tracing::warn!(
                                "Invalid date calculated for monthly frequency: {}-01-01",
                                d.year() + 1
                            );
                            today
                        })
                    } else {
                        chrono::NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1).unwrap_or_else(
                            || {
                                tracing::warn!(
                                    "Invalid date calculated for monthly frequency: {}-{}-01",
                                    d.year(),
                                    d.month() + 1
                                );
                                today
                            },
                        )
                    }
                })
                .unwrap_or_else(|| {
                    start_from.unwrap_or_else(|| {
                        tracing::warn!("Failed to determine start date for missed periods");
                        today
                    })
                });

            let mut current = start_date;
            let mut current_year = current.year();
            let mut current_month = current.month();

            while current <= today {
                let days_in_month: u32 = if current_month == 12 {
                    if let Some(date) = chrono::NaiveDate::from_ymd_opt(current_year + 1, 1, 1) {
                        date.pred_opt().map_or(0, |d| d.day())
                    } else {
                        tracing::warn!(
                            "Invalid date calculated for monthly frequency: {}-12-31",
                            current_year + 1
                        );
                        break;
                    }
                } else if let Some(date) =
                    chrono::NaiveDate::from_ymd_opt(current_year, current_month + 1, 1)
                {
                    date.pred_opt().map_or(0, |d| d.day())
                } else {
                    tracing::warn!(
                        "Invalid date calculated for monthly frequency: {}-{}-01",
                        current_year,
                        current_month + 1
                    );
                    break;
                };

                let day: u32 = days_in_month.min(u32::from(target_day));
                let check_date = chrono::NaiveDate::from_ymd_opt(current_year, current_month, day);

                if let Some(d) = check_date
                    && d <= today
                    && created_date.is_none_or(|created| d >= created)
                {
                    missed.push(format_period_for_monthly(d));
                }

                if current_month == 12 {
                    current_month = 1;
                    current_year += 1;
                } else {
                    current_month += 1;
                }
                current = if let Some(date) =
                    chrono::NaiveDate::from_ymd_opt(current_year, current_month, 1)
                {
                    date
                } else {
                    tracing::warn!(
                        "Invalid date calculated for monthly frequency: {}-{}-01",
                        current_year,
                        current_month
                    );
                    break;
                };
            }
        }
        Frequency::Yearly => {
            let target_month = schedule.month_of_year.unwrap_or(1);
            let target_day = schedule.day_of_month.unwrap_or(1);

            let last_year = last_period.map(|d| d.year()).unwrap_or_else(|| {
                tracing::warn!("Failed to determine last year for yearly frequency");
                today.year() - 1
            });

            let mut current_year = last_year + 1;
            while current_year <= today.year() {
                let check_date = chrono::NaiveDate::from_ymd_opt(
                    current_year,
                    u32::from(target_month),
                    u32::from(target_day),
                );

                if let Some(d) = check_date
                    && d <= today
                    && created_date.is_none_or(|created| d >= created)
                {
                    missed.push(d.format("%Y").to_string());
                }
                current_year += 1;
            }
        }
        Frequency::Quarterly => {
            let _target_month = schedule.month_of_year.unwrap_or(1);
            let target_day = schedule.day_of_month.unwrap_or(1);
            let mut current_year = last_period
                .map(|d| d.year())
                .unwrap_or(start_from.unwrap_or(today).year());
            let mut current_quarter = last_period.map(|d| ((d.month() - 1) / 3) + 1).unwrap_or(1);

            while current_year < today.year()
                || (current_year == today.year()
                    && current_quarter <= ((today.month() - 1) / 3 + 1))
            {
                let quarter_month = (current_quarter - 1) * 3 + 1;
                let check_date = chrono::NaiveDate::from_ymd_opt(
                    current_year,
                    quarter_month,
                    u32::from(target_day),
                );

                if let Some(d) = check_date
                    && d <= today
                    && created_date.is_none_or(|created| d >= created)
                {
                    missed.push(format!("{}/{:04}", current_quarter, current_year));
                }

                if current_quarter == 4 {
                    current_quarter = 1;
                    current_year += 1;
                } else {
                    current_quarter += 1;
                }
            }
        }
    }

    missed
}

#[must_use]
fn format_period_for_date(date: chrono::NaiveDate) -> String {
    format!("{:02}/{:02}/{:04}", date.day(), date.month(), date.year())
}

#[must_use]
fn format_period_for_weekly(date: chrono::NaiveDate) -> String {
    let days_since_monday = date.weekday().num_days_from_sunday();
    let week_start = date - chrono::Duration::days(i64::from(days_since_monday));
    let week_end = week_start + chrono::Duration::days(6);
    format!(
        "{}-{}/{}/{:04}",
        week_start.day(),
        week_end.day(),
        week_end.month(),
        week_end.year()
    )
}

#[must_use]
fn format_period_for_monthly(date: chrono::NaiveDate) -> String {
    format!("{:02}/{:04}", date.month(), date.year())
}

#[must_use]
pub fn get_available_from_datetime(schedule: &Schedule, period: &str) -> Option<String> {
    let target_date = compute_due_date_for_period(schedule, period)?;

    match schedule.frequency {
        Frequency::Daily => {
            if let Some(from_str) = &schedule.available_from_time
                && let Some((hour, minute)) = parse_time_string(from_str)
            {
                let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                    target_date.and_hms_opt(hour, minute, 0)?,
                    chrono::Utc,
                );
                return Some(datetime.to_rfc3339());
            }
            let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                target_date.and_hms_opt(8, 0, 0)?,
                chrono::Utc,
            );
            Some(datetime.to_rfc3339())
        }
        Frequency::Weekly | Frequency::Monthly | Frequency::Yearly => {
            let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                target_date.and_hms_opt(0, 0, 0)?,
                chrono::Utc,
            );
            Some(datetime.to_rfc3339())
        }
        Frequency::Quarterly => {
            let from_hour = schedule
                .available_from_time
                .as_ref()
                .and_then(|s| parse_time_string(s).map(|(h, _)| h))
                .unwrap_or(8);
            let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                target_date.and_hms_opt(from_hour, 0, 0)?,
                chrono::Utc,
            );
            Some(datetime.to_rfc3339())
        }
    }
}

#[must_use]
pub fn get_due_at_datetime(schedule: &Schedule, period: &str) -> Option<String> {
    let target_date = compute_due_date_for_period(schedule, period)?;

    match schedule.frequency {
        Frequency::Daily => {
            if let Some(due_str) = &schedule.due_at_time
                && let Some((hour, minute)) = parse_time_string(due_str)
            {
                let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                    target_date.and_hms_opt(hour, minute, 0)?,
                    chrono::Utc,
                );
                return Some(datetime.to_rfc3339());
            }
            let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                target_date.and_hms_opt(17, 0, 0)?,
                chrono::Utc,
            );
            Some(datetime.to_rfc3339())
        }
        Frequency::Weekly | Frequency::Monthly | Frequency::Yearly => {
            let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                target_date.and_hms_opt(23, 59, 0)?,
                chrono::Utc,
            );
            Some(datetime.to_rfc3339())
        }
        Frequency::Quarterly => {
            let due_hour = schedule
                .due_at_time
                .as_ref()
                .and_then(|s| parse_time_string(s).map(|(h, _)| h))
                .unwrap_or(17);
            let datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                target_date.and_hms_opt(due_hour, 59, 0)?,
                chrono::Utc,
            );
            Some(datetime.to_rfc3339())
        }
    }
}

/// Formats the period string for a given frequency.
///
/// # Panics
/// Panics if week boundary calculations fail.
#[must_use]
pub fn format_period_for_frequency(frequency: &Frequency) -> String {
    let now = chrono::Utc::now();

    match frequency {
        Frequency::Daily => now.format("%d/%m/%Y").to_string(),
        Frequency::Weekly => {
            let days_since_monday = now.weekday().num_days_from_sunday();
            let week_start = if let Some(dt) = (now.date_naive()
                - chrono::Duration::days(i64::from(days_since_monday)))
            .and_hms_opt(0, 0, 0)
            {
                dt.and_utc()
            } else {
                tracing::warn!("Error finding week start date: {now}");
                // Fall back to a simple date format
                return now.format("%d/%m/%Y").to_string();
            };
            let week_end = if let Some(dt) =
                (week_start.date_naive() + chrono::Duration::days(6)).and_hms_opt(23, 59, 59)
            {
                dt.and_utc()
            } else {
                tracing::warn!("Error finding week end date: {now}");
                // Fall back to a simple date format
                return now.format("%d/%m/%Y").to_string();
            };

            format!(
                "{}-{}/{}/{}",
                week_start.day(),
                week_end.day(),
                week_end.month(),
                week_end.format("%Y")
            )
        }
        Frequency::Monthly => now.format("%m/%Y").to_string(),
        Frequency::Quarterly => {
            let quarter = ((now.month() - 1) / 3) + 1;
            format!("{}/{}", quarter, now.format("%Y"))
        }
        Frequency::Yearly => now.format("%Y").to_string(),
    }
}

#[must_use]
pub fn process_template_layout_with_period(
    layout: &TemplateLayout,
    frequency: &Frequency,
) -> TemplateLayout {
    let period = format_period_for_frequency(frequency);

    layout
        .iter()
        .map(|field| {
            let mut processed_field = field.clone();
            if let Some(text) = &field.props.text
                && text.contains("{period}")
            {
                let new_text = text.replace("{period}", &period);
                tracing::info!(
                    "Field type: {}, Original text: '{}', Replaced text: '{}'",
                    field.field_type,
                    text,
                    new_text
                );
                processed_field.props.text = Some(new_text);
            }
            processed_field
        })
        .collect()
}

#[must_use]
pub fn process_template_layout_with_period_string(
    layout: &TemplateLayout,
    period: &str,
) -> TemplateLayout {
    layout
        .iter()
        .map(|field| {
            let mut processed_field = field.clone();
            if let Some(text) = &field.props.text {
                processed_field.props.text = Some(text.replace("{period}", period));
            }
            processed_field
        })
        .collect()
}
