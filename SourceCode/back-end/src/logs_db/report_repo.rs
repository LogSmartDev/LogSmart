use anyhow::Result;
use mongodb::options::ReturnDocument;

use super::types::ReportRunDocument;
use super::{LogDb, collect_cursor};

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
