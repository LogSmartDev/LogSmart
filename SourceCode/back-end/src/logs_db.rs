use anyhow::Result;
use chrono::Datelike;
use chrono::Timelike;
use futures_util::TryStreamExt;
use mongodb::bson::Uuid;
use mongodb::options::ReturnDocument;
use schemars::JsonSchema;
use utoipa::ToSchema;

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

/// Computes the start and end timestamps for the current period based on frequency.
/// Extracted from the three functions that duplicated this ~80-line match block.
fn compute_period_bounds(frequency: &Frequency) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub struct TemplateFieldProps {
    pub text: Option<String>,
    pub size: Option<String>,
    pub weight: Option<String>,
    pub value: Option<String>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub unit: Option<String>,
    pub selected: Option<String>,
    pub options: Option<Vec<String>>,
    pub editable: Option<bool>,
    pub placeholder: Option<String>,
    pub font_family: Option<String>,
    pub text_decoration: Option<String>,
    pub color: Option<String>,
    pub required: Option<bool>,
    pub max_length: Option<i32>,
    pub min_length: Option<i32>,
    pub input_type: Option<String>,
}

impl Default for TemplateFieldProps {
    fn default() -> Self {
        TemplateFieldProps {
            text: None,
            size: None,
            weight: None,
            value: None,
            min: None,
            max: None,
            unit: None,
            selected: None,
            options: None,
            editable: Some(true),
            placeholder: None,
            font_family: None,
            text_decoration: None,
            color: None,
            required: Some(false),
            max_length: None,
            min_length: None,
            input_type: Some("text".to_string()),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub struct TemplateField {
    pub field_type: String,
    pub position: Position,
    pub props: TemplateFieldProps,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema, JsonSchema)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
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

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct Schedule {
    pub frequency: Frequency,
    pub days_of_week: Option<Vec<u8>>,
    pub day_of_week: Option<u8>,
    pub day_of_month: Option<u8>,
    pub month_of_year: Option<u8>,
    #[serde(default)]
    pub available_from_time: Option<String>,
    #[serde(default)]
    pub due_at_time: Option<String>,
}

pub type TemplateLayout = Vec<TemplateField>;

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct TemplateDocument {
    pub template_name: String,
    pub template_layout: TemplateLayout,
    pub company_id: String,
    pub branch_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub schedule: Schedule,
    pub created_by: Uuid,
    #[serde(default = "default_version")]
    pub version: u16,
    pub version_name: Option<String>,
}

impl TemplateDocument {
    #[must_use]
    pub fn is_company_wide(&self) -> bool {
        self.branch_id.is_none()
    }
}

fn default_version() -> u16 {
    1
}

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct TemplateVersionDocument {
    pub template_name: String,
    pub company_id: String,
    pub branch_id: Option<String>,
    pub version: u16,
    pub version_name: Option<String>,
    pub template_layout: TemplateLayout,
    pub schedule: Schedule,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Uuid,
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

async fn backfill_missing_report_params_keys(
    collection: &mongodb::Collection<ReportRunDocument>,
    user_id: &str,
    company_id: &str,
) -> Result<()> {
    let mut cursor = collection
        .find(mongodb::bson::doc! {
            "user_id": user_id,
            "company_id": company_id,
            "$or": [
                { "params_key": { "$exists": false } },
                { "params_key": "" }
            ]
        })
        .await?;

    while let Some(candidate) = cursor.try_next().await? {
        let computed_key = report_params_key(&candidate.params)?;

        collection
            .update_one(
                mongodb::bson::doc! {
                    "report_id": &candidate.report_id,
                    "user_id": user_id,
                    "company_id": company_id,
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

/// Adds a new log template to the database.
///
/// # Errors
/// Returns an error if the database operation fails.
pub async fn add_template(client: &mongodb::Client, template: &TemplateDocument) -> Result<()> {
    let db = LogDb::new(client);
    db.templates().insert_one(template).await?;
    Ok(())
}

/// Adds a new template version to the database.
///
/// # Errors
/// Returns an error if the database operation fails.
pub async fn add_template_version(
    client: &mongodb::Client,
    version_doc: &TemplateVersionDocument,
) -> Result<()> {
    let db = LogDb::new(client);
    db.template_versions().insert_one(version_doc).await?;
    Ok(())
}

/// Retrieves all versions for a specific template.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_template_versions(
    client: &mongodb::Client,
    company_id: &str,
    template_name: &str,
) -> Result<Vec<TemplateVersionDocument>> {
    let db = LogDb::new(client);
    let collection = db.template_versions();

    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "template_name": template_name,
    };

    let find_options = mongodb::options::FindOptions::builder()
        .sort(mongodb::bson::doc! { "version": -1 })
        .build();

    let cursor = collection.find(filter).with_options(find_options).await?;
    collect_cursor(cursor).await
}

/// Retrieves a specific template version.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_template_version(
    client: &mongodb::Client,
    company_id: &str,
    template_name: &str,
    version: u16,
) -> Result<Option<TemplateVersionDocument>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "template_name": template_name,
        "version": u32::from(version),
    };

    db.template_versions().find_one(filter).await.map_err(Into::into)
}

/// Retrieves a log template by its name and company ID.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_template_by_name(
    client: &mongodb::Client,
    template_name: &str,
    company_id: &str,
) -> Result<Option<TemplateDocument>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "template_name": template_name,
        "company_id": company_id,
    };

    db.templates().find_one(filter).await.map_err(Into::into)
}

pub async fn get_templates_by_company(
    client: &mongodb::Client,
    company_id: &str,
) -> Result<Vec<TemplateDocument>> {
    get_templates_by_company_and_branch(client, company_id, None).await
}

/// Retrieves all log templates for a specific company, optionally filtered by branch.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_templates_by_company_and_branch(
    client: &mongodb::Client,
    company_id: &str,
    branch_id: Option<&str>,
) -> Result<Vec<TemplateDocument>> {
    let db = LogDb::new(client);
    let collection = db.templates();

    let filter = if let Some(bid) = branch_id {
        mongodb::bson::doc! {
            "company_id": company_id,
            "$or": [
                { "branch_id": bid },
                { "branch_id": null }
            ]
        }
    } else {
        mongodb::bson::doc! {
            "company_id": company_id,
        }
    };

    let cursor = collection.find(filter).await?;
    collect_cursor(cursor).await
}

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

/// Updates an existing log template.
///
/// # Errors
/// Returns an error if the database update fails.
pub async fn update_template(
    client: &mongodb::Client,
    template_name: &str,
    company_id: &str,
    schedule: Option<&Schedule>,
    layout: Option<&TemplateLayout>,
    version_name: Option<String>,
    branch_id: Option<Option<&str>>,
) -> Result<()> {
    if schedule.is_none() && layout.is_none() && version_name.is_none() && branch_id.is_none() {
        return Ok(());
    }
    let db = LogDb::new(client);
    let collection = db.templates();

    let filter = mongodb::bson::doc! {
        "template_name": template_name,
        "company_id": company_id,
    };

    let mut set_doc = mongodb::bson::Document::new();

    if let Some(schedule) = schedule {
        set_doc.insert("schedule", mongodb::bson::to_bson(&schedule)?);
    }
    if let Some(layout) = layout {
        set_doc.insert("template_layout", mongodb::bson::to_bson(&layout)?);
    }
    if let Some(name) = version_name {
        set_doc.insert("version_name", name);
    }
    if let Some(branch_id) = branch_id {
        let branch_value = match branch_id {
            Some(branch_id) => mongodb::bson::Bson::String(branch_id.to_string()),
            None => mongodb::bson::Bson::Null,
        };
        set_doc.insert("branch_id", branch_value);
    }
    set_doc.insert("updated_at", mongodb::bson::to_bson(&chrono::Utc::now())?);

    let update = mongodb::bson::doc! {
        "$set": set_doc,
        "$inc": { "version": 1 }
    };

    collection.update_one(filter, update).await?;
    Ok(())
}

/// Updates an existing log template with specific version increment.
///
/// # Errors
/// Returns an error if the database update fails.
pub async fn update_template_with_version(
    client: &mongodb::Client,
    template_name: &str,
    company_id: &str,
    schedule: Option<&Schedule>,
    layout: Option<&TemplateLayout>,
) -> Result<()> {
    // This is now redundant given update_template handles versioning,
    // but we can keep the original signature compatible by forwarding
    update_template(
        client,
        template_name,
        company_id,
        schedule,
        layout,
        None,
        None,
    )
    .await
}

/// Renames a log template.
///
/// # Errors
/// Returns an error if a template with the new name already exists or if the database update fails.
pub async fn rename_template(
    client: &mongodb::Client,
    old_name: &str,
    new_name: &str,
    company_id: &str,
) -> Result<()> {
    let db = LogDb::new(client);
    let collection = db.templates();

    let existing_template = collection
        .find_one(mongodb::bson::doc! {
            "template_name": new_name,
            "company_id": company_id,
        })
        .await?
        .is_some();

    if existing_template {
        anyhow::bail!("Template with the new name already exists");
    }

    let filter = mongodb::bson::doc! {
        "template_name": old_name,
        "company_id": company_id,
    };

    let update = mongodb::bson::doc! {
        "$set": {
            "template_name": new_name,
            "updated_at": mongodb::bson::to_bson(&chrono::Utc::now())?,
        }
    };

    collection.update_one(filter, update).await?;
    Ok(())
}

/// Deletes a log template.
///
/// # Errors
/// Returns an error if the database deletion fails.
pub async fn delete_template(
    client: &mongodb::Client,
    template_name: &str,
    company_id: &str,
) -> Result<()> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "template_name": template_name,
        "company_id": company_id,
    };

    db.templates().delete_one(filter).await?;
    Ok(())
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct LogEntry {
    pub entry_id: String,
    pub template_name: String,
    pub company_id: String,
    pub branch_id: Option<String>,
    pub user_id: String,
    pub entry_data: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: LogStatus,
    pub period: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ReportRunDocument {
    pub report_id: String,
    pub user_id: String,
    pub company_id: String,
    pub name: Option<String>,
    pub params: crate::dto::ReportRunParams,
    #[serde(default)]
    pub params_key: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub use_count: u32,
}

#[must_use]
pub fn normalize_report_params(
    params: &crate::dto::ReportRunParams,
) -> crate::dto::ReportRunParams {
    let mut normalized = params.clone();
    normalized.selected_branch_ids.sort();
    normalized.selected_branch_ids.dedup();
    normalized.selected_log_type_ids.sort();
    normalized.selected_log_type_ids.dedup();
    normalized
}

pub fn report_params_key(params: &crate::dto::ReportRunParams) -> Result<String> {
    let normalized = normalize_report_params(params);
    serde_json::to_string(&normalized).map_err(Into::into)
}

/// Creates a saved report run for a user.
///
/// # Errors
/// Returns an error if insertion fails.
pub async fn create_report_run(
    client: &mongodb::Client,
    report_run: &ReportRunDocument,
) -> Result<ReportRunDocument> {
    let db = LogDb::new(client);
    let collection = db.report_runs();

    let normalized_params = normalize_report_params(&report_run.params);
    let params_key = report_params_key(&normalized_params)?;

    backfill_missing_report_params_keys(&collection, &report_run.user_id, &report_run.company_id)
        .await?;

    let filter = mongodb::bson::doc! {
        "user_id": &report_run.user_id,
        "company_id": &report_run.company_id,
        "params_key": &params_key,
    };

    let existing = collection.find_one(filter).await?;

    if let Some(existing) = existing {
        let mut set_doc = mongodb::bson::Document::new();
        set_doc.insert("last_used_at", mongodb::bson::to_bson(&chrono::Utc::now())?);
        set_doc.insert("params_key", mongodb::bson::to_bson(&params_key)?);
        set_doc.insert("params", mongodb::bson::to_bson(&normalized_params)?);

        if let Some(name) = &report_run.name {
            set_doc.insert("name", mongodb::bson::to_bson(name)?);
        }

        let updated = collection
            .find_one_and_update(
                mongodb::bson::doc! {
                    "report_id": &existing.report_id,
                    "user_id": &existing.user_id,
                    "company_id": &existing.company_id,
                },
                mongodb::bson::doc! {
                    "$set": set_doc,
                    "$inc": {
                        "use_count": 1,
                    }
                },
            )
            .return_document(ReturnDocument::After)
            .await?;

        if let Some(updated) = updated {
            return Ok(updated);
        }

        return Ok(existing);
    }

    let mut to_insert = report_run.clone();
    to_insert.params = normalized_params;
    to_insert.params_key = params_key;

    collection.insert_one(&to_insert).await?;
    Ok(to_insert)
}

/// Lists saved report runs for a user.
///
/// # Errors
/// Returns an error if query fails.
pub async fn list_report_runs(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
    limit: i64,
) -> Result<Vec<ReportRunDocument>> {
    let db = LogDb::new(client);
    let collection = db.report_runs();

    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
    };

    let cursor = collection
        .find(filter)
        .sort(mongodb::bson::doc! { "last_used_at": -1, "created_at": -1 })
        .limit(limit)
        .await?;

    collect_cursor(cursor).await
}

/// Marks a saved report run as used.
///
/// # Errors
/// Returns an error if update fails.
pub async fn touch_report_run(
    client: &mongodb::Client,
    report_id: &str,
    user_id: &str,
    company_id: &str,
) -> Result<bool> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "report_id": report_id,
        "user_id": user_id,
        "company_id": company_id,
    };

    let update = mongodb::bson::doc! {
        "$set": {
            "last_used_at": mongodb::bson::to_bson(&chrono::Utc::now())?,
        },
        "$inc": {
            "use_count": 1,
        }
    };

    let result = db.report_runs().update_one(filter, update).await?;
    Ok(result.matched_count > 0)
}

/// Deletes a saved report run.
///
/// # Errors
/// Returns an error if delete fails.
pub async fn delete_report_run(
    client: &mongodb::Client,
    report_id: &str,
    user_id: &str,
    company_id: &str,
) -> Result<bool> {
    let db = LogDb::new(client);
    let collection = db.report_runs();

    let id_filter = mongodb::bson::doc! {
        "report_id": report_id,
        "user_id": user_id,
        "company_id": company_id,
    };

    let Some(target) = collection.find_one(id_filter).await? else {
        tracing::warn!(
            target: "report_runs",
            "mongo delete: target not found for report_id={}, user_id={}, company_id={}",
            report_id,
            user_id,
            company_id
        );
        return Ok(false);
    };

    let params_key = if target.params_key.is_empty() {
        report_params_key(&target.params)?
    } else {
        target.params_key
    };

    backfill_missing_report_params_keys(&collection, user_id, company_id).await?;

    tracing::info!(
        target: "report_runs",
        "mongo delete: resolved params_key for report_id={} key_len={}",
        report_id,
        params_key.len()
    );

    let result = collection
        .delete_many(mongodb::bson::doc! {
            "user_id": user_id,
            "company_id": company_id,
            "params_key": &params_key,
        })
        .await?;

    tracing::info!(
        target: "report_runs",
        "mongo delete: deleted_count={} for report_id={} user_id={} company_id={}",
        result.deleted_count,
        report_id,
        user_id,
        company_id
    );
    Ok(result.deleted_count > 0)
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

pub async fn get_user_log_entries_by_template(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
    template_name: &str,
) -> Result<Vec<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
        "template_name": template_name,
    };

    let cursor = db.log_entries().find(filter).await?;
    collect_cursor(cursor).await
}

/// Retrieves the most recently submitted log entry for a user and template.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_latest_submitted_entry(
    client: &mongodb::Client,
    user_id: &str,
    company_id: &str,
    template_name: &str,
) -> Result<Option<LogEntry>> {
    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
        "template_name": template_name,
        "status": mongodb::bson::to_bson(&LogStatus::Submitted)?,
    };

    db.log_entries()
        .find_one(filter)
        .sort(mongodb::bson::doc! { "submitted_at": -1 })
        .await
        .map_err(Into::into)
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
/// Uses a simple approach: fetches all submitted entries for the current period for all templates,
/// then builds a lookup map.
///
/// # Errors
/// Returns an error if the database query fails.
///
/// # Panics
/// Panics if period boundary calculations fail.
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
    let now = chrono::Utc::now();
    let period_start = now - chrono::Duration::days(400);
    let period_end = now;

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
    for (name, _) in templates {
        let has_submitted = entries.iter().any(|e| {
            e.template_name == *name && is_in_current_period(&e.submitted_at, &e.period, now)
        });
        results.insert(name.to_string(), has_submitted);
    }

    Ok(results)
}

fn is_in_current_period(
    submitted_at: &Option<chrono::DateTime<chrono::Utc>>,
    _period: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let Some(ts) = submitted_at else {
        return false;
    };

    ts.date_naive() == now.date_naive()
}

/// Batch version of get_draft_entry_for_current_period - gets draft entries for multiple templates.
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
    let filter = mongodb::bson::doc! {
        "user_id": user_id,
        "company_id": company_id,
        "template_name": { "$in": template_names },
        "status": mongodb::bson::to_bson(&LogStatus::Draft)?,
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

pub async fn get_periods_with_entries(
    client: &mongodb::Client,
    company_id: &str,
    template_name: &str,
    periods: &[String],
) -> Result<std::collections::HashSet<String>> {
    if periods.is_empty() {
        return Ok(std::collections::HashSet::new());
    }

    let db = LogDb::new(client);
    let filter = mongodb::bson::doc! {
        "company_id": company_id,
        "template_name": template_name,
        "period": { "$in": periods }
    };

    let cursor = db.log_entries().find(filter).await?;
    let entries: Vec<LogEntry> = cursor.try_collect().await?;
    Ok(entries.into_iter().map(|e| e.period).collect())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityStatus {
    NotAvailable,
    Available,
    Overdue,
}

impl AvailabilityStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            AvailabilityStatus::NotAvailable => "not_available",
            AvailabilityStatus::Available => "available",
            AvailabilityStatus::Overdue => "overdue",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum LogStatus {
    #[default]
    Draft,
    Submitted,
    Reviewed,
    Approved,
    Overdue,
}

impl LogStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            LogStatus::Draft => "draft",
            LogStatus::Submitted => "submitted",
            LogStatus::Reviewed => "reviewed",
            LogStatus::Approved => "approved",
            LogStatus::Overdue => "overdue",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(LogStatus::Draft),
            "submitted" => Some(LogStatus::Submitted),
            "reviewed" => Some(LogStatus::Reviewed),
            "approved" => Some(LogStatus::Approved),
            "overdue" => Some(LogStatus::Overdue),
            _ => None,
        }
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

#[must_use]
pub fn compute_due_date_for_period(schedule: &Schedule, period: &str) -> Option<chrono::NaiveDate> {
    let parts: Vec<&str> = period.split('/').collect();

    fn normalize_year(year: i32) -> Option<i32> {
        if year >= 2000 { Some(year) } else { None }
    }

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

    fn normalize_year(year: i32) -> Option<i32> {
        if year >= 2000 { Some(year) } else { None }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodValidationError {
    FormatInvalid,
    DueDateInFuture,
    WeekdayNotAllowed,
    BeforeTemplateCreation,
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
                } else {
                    if let Some(date) =
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
                    }
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
