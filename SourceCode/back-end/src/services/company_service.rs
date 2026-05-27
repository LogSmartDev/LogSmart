use crate::{db, error::AppError, images_db, utils::infer_content_type};
use sqlx::PgPool;

pub struct CompanyService;

impl CompanyService {
    pub async fn upload_company_logo(
        postgres: &PgPool,
        mongodb: &mongodb::Client,
        company_id: &str,
        data: Vec<u8>,
    ) -> Result<String, AppError> {
        if data.len() > 10 * 1024 * 1024 {
            return Err(AppError::BadRequest(
                "File too large. Maximum size is 10MB".to_string(),
            ));
        }

        if data.is_empty() {
            return Err(AppError::BadRequest("No file provided".to_string()));
        }

        let content_type = infer_content_type(&data);
        if !content_type.starts_with("image/") {
            return Err(AppError::BadRequest("File must be an image".to_string()));
        }

        let company = db::get_company_by_id(postgres, company_id)
            .await
            .map_err(|e| AppError::Internal(format!("Database error: {e}")))?
            .ok_or(AppError::NotFound("Company not found".to_string()))?;

        let file_id = images_db::upload_company_logo(mongodb, data, company_id, &content_type)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to upload logo: {e}")))?;

        if let Err(err) = db::update_company_logo_id(postgres, company_id, Some(&file_id)).await {
            tracing::error!("Failed to update company logo: {:?}", err);
            if let Err(delete_err) = images_db::delete_company_logo(mongodb, &file_id).await {
                tracing::error!("Failed to cleanup uploaded logo: {:?}", delete_err);
            }
            return Err(AppError::Internal(
                "Failed to update logo reference".to_string(),
            ));
        }

        if let Some(old_logo_id) = &company.logo_id
            && let Err(err) = images_db::delete_company_logo(mongodb, old_logo_id).await
        {
            tracing::error!("Failed to delete old company logo: {:?}", err);
        }

        Ok(file_id)
    }
}
