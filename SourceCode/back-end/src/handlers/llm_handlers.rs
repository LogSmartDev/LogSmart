use crate::AppState;
use crate::dto::LayoutGenerationRequest;
use crate::llm::{self};
use crate::middleware::AnyAuthUser;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

#[utoipa::path(
    post,
    path = "/llm/generate-layout",
    tag = "LLM",
    request_body = LayoutGenerationRequest,
    responses(
        (status = 200, description = "Layout generated successfully", body = serde_json::Value),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_token" = []))
)]
/// Generates a log layout using LLM based on user prompt.
///
/// # Errors
/// Returns an error if the user prompt is empty or if LLM generation fails.
pub async fn generate_layout(
    AnyAuthUser(_claims, _user): AnyAuthUser,
    State(_state): State<AppState>,
    Json(req): Json<LayoutGenerationRequest>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    if req.user_prompt.trim().is_empty() {
        return Err(crate::error::AppError::BadRequest("User prompt cannot be empty".to_string()));
    }

    if req.user_prompt.len() > 1000 {
        return Err(crate::error::AppError::BadRequest("User prompt is too long (max 1000 characters)".to_string()));
    }

    match llm::generate_layout(req).await {
        Ok(response) => Ok((
            StatusCode::OK,
            Json(json!({
                "layout": response.layout
            })),
        )),
        Err(e) => {
            tracing::error!("LLM generation error: {}", e);
            Err(crate::error::AppError::Internal("Failed to generate layout"
                .to_string()))
        }
    }
}
