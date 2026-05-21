use anyhow::Result;
use mongodb::options::ReturnDocument;
use futures_util::TryStreamExt;

use super::types::{Frequency, LogEntry, LogStatus};
use super::{LogDb, collect_cursor};
use super::scheduling::compute_period_bounds;

/// Retrieves all log entries for a branch.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_branch_log_entries(
    client: &mongodb::Client,
    company_id: &str,
    branch_id: &str,
) -> Result<Vec<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "branch_id": branch_id,
    };

    let cursor = db.log_entries().find(filter).await?;
    collect_cursor(cursor).await
}

pub async fn get_company_log_entries(
    client: &mongodb::Client,
    company_id: &str,
) -> Result<Vec<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "company_id": company_id,
    };

    let cursor = db.log_entries().find(filter).await?;
    collect_cursor(cursor).await
}

pub async fn get_branches_log_entries(
    client: &mongodb::Client,
    company_id: &str,
    branch_ids: &[String],
) -> Result<Vec<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "branch_id": mongodb::bson::doc! { "$in": branch_ids }
    };

    let cursor = db.log_entries().find(filter).await?;
    collect_cursor(cursor).await
}

/// Creates a new log entry.
///
/// # Errors
/// Returns an error if the database operation fails.
pub async fn create_log_entry(client: &mongodb::Client, entry: &LogEntry) -> Result<()> {
    let db = LogDb::new(client);
    db.log_entries().insert_one(entry).await?;
    Ok(())
}

pub async fn get_log_entry(client: &mongodb::Client, entry_id: &str) -> Result<Option<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "entry_id": entry_id,
    };

    db.log_entries().find_one(filter).await.map_err(Into::into)
}

pub async fn get_user_log_entries(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
) -> Result<Vec<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
    };

    let cursor = db.log_entries().find(filter).await?;
    collect_cursor(cursor).await
}

/// Batch version of get_latest_submitted_entry - gets latest submitted entry for multiple templates.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_latest_submitted_entries_batch(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
    template_names: &[String],
) -> Result<std::collections::HashMap<String, LogEntry>> {
    if template_names.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
        "template_name": { "$in": template_names },
        "status": mongodb::bson::to_bson(&LogStatus::Submitted)?,
    };

    let cursor = db.log_entries()
        .find(filter)
        .sort(mongodb::bson::doc! { "submitted_at": -1 })
        .await?;
    let entries = collect_cursor(cursor).await?;

    let mut results = std::collections::HashMap::new();
    for entry in entries {
        results.entry(entry.template_name.clone()).or_insert(entry);
    }

    Ok(results)
}

/// Batch version of get_periods_with_entries - gets periods with entries for multiple templates.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_periods_with_entries_batch(
    client: &mongodb::Client,
    company_id: &str,
    template_names: &[String],
    all_periods: &[String],
) -> Result<std::collections::HashMap<String, std::collections::HashSet<String>>> {
    if template_names.is_empty() || all_periods.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "template_name": { "$in": template_names },
        "period": { "$in": all_periods }
    };

    let cursor = db.log_entries().find(filter).await?;
    let entries: Vec<LogEntry> = cursor.try_collect().await?;

    let mut results: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    for entry in entries {
        results
            .entry(entry.template_name)
            .or_default()
            .insert(entry.period);
    }

    Ok(results)
}

/// Batch version of has_submitted_entry_for_current_period - checks multiple templates at once.
///
/// Uses a single query with a wide-enough date range, then validates each entry
/// against its template's exact period boundaries in memory.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn has_submitted_entries_batch(
    client: &mongodb::Client,
    company_id: &str,
    templates: &[(&str, &Frequency)],
) -> Result<std::collections::HashMap<String, bool>> {
    if templates.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let db = LogDb::new(client);
    let template_names: Vec<&str> = templates.iter().map(|(n, _)| *n).collect();

    // Compute the widest possible period bounds across all frequencies
    let (period_start, period_end) = {
        let daily = compute_period_bounds(&Frequency::Daily);
        let yearly = compute_period_bounds(&Frequency::Yearly);
        let start = std::cmp::min(daily.0, yearly.0);
        let end = std::cmp::max(daily.1, yearly.1);
        (start, end)
    };

    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "template_name": { "$in": template_names },
        "submitted_at": {
            "$gte": mongodb::bson::DateTime::from_system_time(std::time::SystemTime::from(period_start)),
            "$lte": mongodb::bson::DateTime::from_system_time(std::time::SystemTime::from(period_end))
        },
        "status": mongodb::bson::to_bson(&LogStatus::Submitted)?,
    };

    let cursor = db.log_entries().find(filter).await?;
    let entries: Vec<LogEntry> = cursor.try_collect().await?;

    let mut results = std::collections::HashMap::new();
    for (name, freq) in templates {
        let (start, end) = compute_period_bounds(freq);
        let has_submitted = entries.iter().any(|e| {
            e.template_name == *name
                && e.submitted_at.is_some_and(|ts| ts >= start && ts <= end)
        });
        results.insert(name.to_string(), has_submitted);
    }

    Ok(results)
}

/// Batch version of get_draft_entry_for_current_period - gets draft entries for multiple templates.
/// Filters to current period only using the widest period bounds.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_draft_entries_batch(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
    template_names: &[String],
) -> Result<std::collections::HashMap<String, Option<LogEntry>>> {
    if template_names.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let db = LogDb::new(client);

    // Use the widest period bounds to capture all current-period drafts in one query
    let (period_start, period_end) = {
        let daily = compute_period_bounds(&Frequency::Daily);
        let yearly = compute_period_bounds(&Frequency::Yearly);
        (std::cmp::min(daily.0, yearly.0), std::cmp::max(daily.1, yearly.1))
    };

    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
        "template_name": { "$in": template_names },
        "status": mongodb::bson::to_bson(&LogStatus::Draft)?,
        "created_at": {
            "$gte": mongodb::bson::DateTime::from_system_time(std::time::SystemTime::from(period_start)),
            "$lte": mongodb::bson::DateTime::from_system_time(std::time::SystemTime::from(period_end)),
        },
    };

    let cursor = db.log_entries().find(filter).await?;
    let entries: Vec<LogEntry> = cursor.try_collect().await?;

    let mut results = std::collections::HashMap::new();
    for name in template_names {
        results.insert(name.clone(), None);
    }
    for entry in entries {
        let name = entry.template_name.clone();
        results.insert(name, Some(entry));
    }

    Ok(results)
}

/// Checks if a log entry exists for the current period and template.
///
/// # Errors
/// Returns an error if the database query fails.
///
/// # Panics
/// Panics if period boundary calculations fail.
pub async fn has_entry_for_current_period(
    client: &mongodb::Client,
    company_id: &str,
    template_name: &str,
    frequency: &Frequency,
) -> Result<bool> {
    let db = LogDb::new(client);
    let (period_start, period_end) = compute_period_bounds(frequency);

    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "template_name": template_name,
        "created_at": {
            "$gte": mongodb::bson::to_bson(&period_start)?,
            "$lte": mongodb::bson::to_bson(&period_end)?,
        },
    };

    let result = db.log_entries().find_one(filter).await?;
    Ok(result.is_some())
}

/// Checks if a submitted log entry exists for the current period and template.
///
/// # Errors
/// Returns an error if the database query fails.
///
/// # Panics
/// Panics if period boundary calculations fail.
pub async fn has_submitted_entry_for_current_period(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
    template_name: &str,
    frequency: &Frequency,
) -> Result<bool> {
    let db = LogDb::new(client);
    let (period_start, period_end) = compute_period_bounds(frequency);

    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
        "template_name": template_name,
        "status": mongodb::bson::to_bson(&LogStatus::Submitted)?,
        "created_at": {
            "$gte": mongodb::bson::to_bson(&period_start)?,
            "$lte": mongodb::bson::to_bson(&period_end)?,
        },
    };

    let result = db.log_entries().find_one(filter).await?;
    Ok(result.is_some())
}

/// Checks if any log entry (draft or submitted) exists for a specific period.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn has_entry_for_period(
    client: &mongodb::Client,
    company_id: &str,
    template_name: &str,
    period: &str,
) -> Result<bool> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "template_name": template_name,
        "period": period,
    };

    let result = db.log_entries().find_one(filter).await?;
    Ok(result.is_some())
}

/// Retrieves a draft log entry for the current period and template.
///
/// # Errors
/// Returns an error if the database query fails.
///
/// # Panics
/// Panics if period boundary calculations fail.
pub async fn get_draft_entry_for_current_period(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
    template_name: &str,
    frequency: &Frequency,
) -> Result<Option<LogEntry>> {
    let db = LogDb::new(client);
    let (period_start, period_end) = compute_period_bounds(frequency);

    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
        "template_name": template_name,
        "status": mongodb::bson::to_bson(&LogStatus::Draft)?,
        "created_at": {
            "$gte": mongodb::bson::to_bson(&period_start)?,
            "$lte": mongodb::bson::to_bson(&period_end)?,
        },
    };

    db.log_entries().find_one(filter).await.map_err(Into::into)
}

/// Updates the data of an existing log entry.
///
/// # Errors
/// Returns an error if the database update fails.
pub async fn update_log_entry(
    client: &mongodb::Client,
    entry_id: &str,
    entry_data: &serde_json::Value,
) -> Result<()> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "entry_id": entry_id,
    };

    let update = mongodb::bson::doc! {
        "$set": {
            "entry_data": mongodb::bson::to_bson(&entry_data)?,
            "updated_at": mongodb::bson::to_bson(&chrono::Utc::now())?,
        }
    };

    db.log_entries().update_one(filter, update).await?;
    Ok(())
}

/// Updates a log entry and returns the updated document in a single round trip.
///
/// # Errors
/// Returns an error if the database update fails.
pub async fn update_log_entry_with_return(
    client: &mongodb::Client,
    entry_id: &str,
    entry_data: &serde_json::Value,
) -> Result<Option<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "entry_id": entry_id,
    };

    let update = mongodb::bson::doc! {
        "$set": {
            "entry_data": mongodb::bson::to_bson(&entry_data)?,
            "updated_at": mongodb::bson::to_bson(&chrono::Utc::now())?,
        }
    };

    let result = db.log_entries()
        .find_one_and_update(filter, update)
        .return_document(ReturnDocument::After)
        .await?;
    Ok(result)
}

pub async fn submit_log_entry(client: &mongodb::Client, entry_id: &str) -> Result<()> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "entry_id": entry_id,
    };

    let update = mongodb::bson::doc! {
        "$set": {
            "status": mongodb::bson::to_bson(&LogStatus::Submitted)?,
            "submitted_at": mongodb::bson::to_bson(&chrono::Utc::now())?,
            "updated_at": mongodb::bson::to_bson(&chrono::Utc::now())?,
        }
    };

    db.log_entries().update_one(filter, update).await?;
    Ok(())
}

pub async fn unsubmit_log_entry(client: &mongodb::Client, entry_id: &str) -> Result<()> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "entry_id": entry_id,
    };

    let update = mongodb::bson::doc! {
        "$set": {
            "status": mongodb::bson::to_bson(&LogStatus::Draft)?,
            "submitted_at": mongodb::bson::Bson::Null,
            "updated_at": mongodb::bson::to_bson(&chrono::Utc::now())?,
        }
    };

    db.log_entries().update_one(filter, update).await?;
    Ok(())
}

pub async fn delete_log_entry(client: &mongodb::Client, entry_id: &str) -> Result<()> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "entry_id": entry_id,
    };

    db.log_entries().delete_one(filter).await?;
    Ok(())
}
