use anyhow::Result;

use super::types::{Schedule, TemplateDocument, TemplateLayout, TemplateVersionDocument};
use super::{LogDb, collect_cursor};

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

    db.template_versions()
        .find_one(filter)
        .await
        .map_err(Into::into)
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
