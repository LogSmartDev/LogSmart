mod entry_repo;
mod report_repo;
mod scheduling;
mod template_repo;
mod types;

pub use types::{
    AvailabilityStatus, Frequency, LogEntry, LogStatus, PeriodValidationError, Position,
    ReportRunDocument, Schedule, TemplateDocument, TemplateField, TemplateFieldProps,
    TemplateLayout, TemplateVersionDocument,
};

pub use entry_repo::{
    create_log_entry, delete_log_entry, get_branch_log_entries, get_branches_log_entries,
    get_company_log_entries, get_draft_entries_batch, get_draft_entry_for_current_period,
    get_latest_submitted_entries_batch, get_log_entry, get_periods_with_entries_batch,
    get_user_log_entries, has_entry_for_current_period, has_entry_for_period,
    has_submitted_entries_batch, has_submitted_entry_for_current_period, submit_log_entry,
    unsubmit_log_entry, update_log_entry, update_log_entry_with_return,
};

pub use report_repo::{
    create_report_run, delete_report_run, list_report_runs, normalize_report_params,
    report_params_key, touch_report_run,
};

pub use scheduling::{
    compute_due_date_for_period, derive_log_status, format_period_for_frequency,
    get_availability_status_for_period, get_available_from_datetime, get_due_at_datetime,
    get_missed_periods, is_form_due_today, parse_period_to_date, parse_time_string,
    process_template_layout_with_period, process_template_layout_with_period_string,
    validate_and_normalize_period, validate_period_business_rules,
};

pub use template_repo::{
    add_template, add_template_version, delete_template, get_template_by_name,
    get_template_version, get_template_versions, get_templates_by_company,
    get_templates_by_company_and_branch, rename_template, update_template,
};

use anyhow::Result;
use futures_util::TryStreamExt;

/// Wrapper around a MongoDB database providing typed collection accessors.
/// Eliminates repeated `client.database("logs_db").collection("...")` boilerplate.
#[derive(Clone)]
pub struct LogDb {
    db: mongodb::Database,
}

impl LogDb {
    #[must_use]
    pub fn new(client: &mongodb::Client) -> Self {
        Self {
            db: client.database("logs_db"),
        }
    }

    pub fn templates(&self) -> mongodb::Collection<TemplateDocument> {
        self.db.collection("templates")
    }

    pub fn template_versions(&self) -> mongodb::Collection<TemplateVersionDocument> {
        self.db.collection("template_versions")
    }

    pub fn log_entries(&self) -> mongodb::Collection<LogEntry> {
        self.db.collection("log_entries")
    }

    pub fn report_runs(&self) -> mongodb::Collection<ReportRunDocument> {
        self.db.collection("report_runs")
    }
}

/// Collects all documents from a MongoDB cursor into a Vec.
/// Eliminates the repeated `while let Some(item) = cursor.try_next().await?` pattern.
pub async fn collect_cursor<S, T>(mut cursor: S) -> Result<Vec<T>>
where
    S: futures_util::TryStream<Ok = T, Error = mongodb::error::Error> + Unpin,
{
    let mut results = Vec::new();
    while let Some(item) = cursor.try_next().await? {
        results.push(item);
    }
    Ok(results)
}

/// Initializes the `MongoDB` client.
///
/// # Errors
/// Returns an error if the connection fails.
///
/// # Panics
/// Panics if `MONGODB_URI` environment variable is not set.
pub async fn init_mongodb() -> Result<mongodb::Client> {
    let mongo_uri = std::env::var("MONGODB_URI").expect("MONGODB_URI not set in environment");
    let client = mongodb::Client::with_uri_str(&mongo_uri)
        .await
        .map_err(anyhow::Error::from)?;

    ensure_report_run_indexes(&client).await?;
    backfill_missing_report_params_keys_all(&client).await?;

    Ok(client)
}

async fn ensure_report_run_indexes(client: &mongodb::Client) -> Result<()> {
    let db = LogDb::new(client);
    let collection = db.report_runs();

    let index = mongodb::IndexModel::builder()
        .keys(mongodb::bson::doc! {
            "user_id": 1,
            "company_id": 1,
            "params_key": 1,
        })
        .options(
            mongodb::options::IndexOptions::builder()
                .name(Some("report_runs_user_company_params_key_idx".to_string()))
                .build(),
        )
        .build();

    collection.create_index(index).await?;

    Ok(())
}

/// One-time startup migration: backfills missing params_key for all report runs.
/// Previously ran on every create/delete (hot path); now runs once at startup.
async fn backfill_missing_report_params_keys_all(client: &mongodb::Client) -> Result<()> {
    let db = LogDb::new(client);
    let collection = db.report_runs();

    let mut cursor = collection
        .find(mongodb::bson::doc! {
            "$or": [
                { "params_key": { "$exists": false } },
                { "params_key": "" }
            ]
        })
        .await?;

    while let Some(candidate) = cursor.try_next().await? {
        let computed_key = report_repo::report_params_key(&candidate.params)?;

        collection
            .update_one(
                mongodb::bson::doc! {
                    "report_id": &candidate.report_id,
                    "$or": [
                        { "params_key": { "$exists": false } },
                        { "params_key": "" }
                    ]
                },
                mongodb::bson::doc! {
                    "$set": {
                        "params_key": computed_key,
                    }
                },
            )
            .await?;
    }

    Ok(())
}
