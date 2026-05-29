use crate::dto::{LayoutGenerationRequest, LayoutGenerationResponse};
use crate::logs_db::TemplateLayout;
use anyhow::{Context, Result, anyhow};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct LayoutOutput {
    pub template_layout: TemplateLayout,
}

const DEFAULT_MODEL: &str = "openrouter/free";
const DEFAULT_OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

const SYSTEM_PROMPT: &str = r#"You are LogSmart's template layout generator.

Return a JSON object for a form layout in this exact shape:
{
  "template_layout": [
    {
      "field_type": "text_input | checkbox | temperature | dropdown | label",
      "position": { "x": number, "y": number },
      "props": { ... }
    }
  ]
}

You will receive the user's request plus context that includes:
- Canvas dimensions
- Current components already on the canvas (their types, positions, and props)

Rules you must follow:
1) Use ONLY supported field types:
   - text_input
   - checkbox
   - temperature
   - dropdown
   - label

2) Positioning with existing components:
   - Use absolute coordinates in pixels via position.x and position.y.
   - The user prompt will include a "Current canvas context" section listing existing components with their positions.
   - You MUST NOT overlap with any existing component. Leave at least 16px gap between your new components and existing ones.
   - When adding new components to a canvas that already has items, place new components BELOW the lowest existing component (add padding of at least 24px).
   - Keep components fully inside canvas bounds.
   - Default to a clean top-to-bottom layout unless the user explicitly asks for columns/grid.
   - Prefer consistent spacing (about 16-24px) between new components.
   - NEVER reposition or modify existing components — only add new ones.

3) Field-specific props:
   - text_input: use props like text, placeholder, required, min_length, max_length, input_type.
   - checkbox: use text and optional required.
   - temperature: include min/max as numbers, optional value, label, and unit.
   - dropdown: include options (non-empty array), optional selected.
   - label: use text; optional style props (size, weight, font_family, text_decoration, color).

4) General props guidance:
   - Use only schema-supported props.
   - Keep props minimal and meaningful; do not invent unknown keys.
   - Use editable=false for display-only label content when appropriate.
   - If a value is uncertain, omit the prop instead of guessing aggressively.

5) Data quality:
   - Produce valid JSON.
   - Ensure numeric fields are numbers (not strings), especially x/y/min/max.
   - Make practical layouts that reflect the user's intent (e.g., checklist, temperature log, inspection form).

Output constraints:
- Return ONLY raw JSON.
- No markdown, no explanations, no code fences.
- No keys outside the schema object."#;

/// Generates a layout using the configured LLM.
///
/// # Errors
/// Returns an error if the LLM request fails or if environment variables are missing.
pub async fn generate_layout(request: LayoutGenerationRequest) -> Result<LayoutGenerationResponse> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY is required for LLM generation")?;

    let model = std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let openrouter_url =
        std::env::var("OPENROUTER_URL").unwrap_or_else(|_| DEFAULT_OPENROUTER_URL.to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .context("Invalid OPENROUTER_API_KEY format")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "HTTP-Referer",
        HeaderValue::from_str(
            &std::env::var("OPENROUTER_REFERER")
                .unwrap_or_else(|_| "https://logsmart.app".to_string()),
        )?,
    );
    headers.insert(
        "X-Title",
        HeaderValue::from_str(
            &std::env::var("OPENROUTER_TITLE").unwrap_or_else(|_| "LogSmart".to_string()),
        )?,
    );

    let schema = schemars::schema_for!(LayoutOutput);
    let schema_value = serde_json::to_value(&schema)?;

    let payload = OpenRouterRequest {
        model,
        messages: vec![
            OpenRouterMessage {
                role: "system",
                content: SYSTEM_PROMPT.to_string(),
            },
            OpenRouterMessage {
                role: "user",
                content: request.user_prompt,
            },
        ],
        response_format: OpenRouterResponseFormat {
            r#type: "json_schema",
            json_schema: OpenRouterJsonSchema {
                name: "layout_output",
                strict: true,
                schema: schema_value,
            },
        },
    };

    let client = reqwest::Client::new();
    let res = client
        .post(openrouter_url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .context("Failed to call OpenRouter")?
        .error_for_status()
        .context("OpenRouter returned an error status")?;

    let completion: OpenRouterResponse = res
        .json()
        .await
        .context("Failed to deserialize OpenRouter response")?;

    let first_choice = completion
        .choices
        .first()
        .ok_or_else(|| anyhow!("OpenRouter returned no choices"))?;

    let content = first_choice
        .message
        .content
        .as_str()
        .ok_or_else(|| anyhow!("OpenRouter response content was not a string"))?;

    let layout: serde_json::Value = serde_json::from_str(content)
        .unwrap_or_else(|_| serde_json::json!({ "template_layout": [] }));

    Ok(LayoutGenerationResponse { layout })
}

#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    response_format: OpenRouterResponseFormat,
}

#[derive(Serialize)]
struct OpenRouterMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct OpenRouterResponseFormat {
    r#type: &'static str,
    json_schema: OpenRouterJsonSchema,
}

#[derive(Serialize)]
struct OpenRouterJsonSchema {
    name: &'static str,
    strict: bool,
    schema: serde_json::Value,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterResponseMessage,
}

#[derive(Deserialize)]
struct OpenRouterResponseMessage {
    content: serde_json::Value,
}
