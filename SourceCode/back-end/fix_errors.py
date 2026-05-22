import os
import re
import glob

def fix_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # 1. Fix map_err(|...| { ... (StatusCode::XXX, json!(...)) }) -> AppError
    content = re.sub(
        r'\(\s*StatusCode::INTERNAL_SERVER_ERROR\s*,\s*json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\),?\s*\)',
        r'crate::error::AppError::Internal(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::NOT_FOUND\s*,\s*json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\),?\s*\)',
        r'crate::error::AppError::NotFound(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::UNAUTHORIZED\s*,\s*json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\),?\s*\)',
        r'crate::error::AppError::Unauthorized(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::FORBIDDEN\s*,\s*json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\),?\s*\)',
        r'crate::error::AppError::Forbidden(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::BAD_REQUEST\s*,\s*json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\),?\s*\)',
        r'crate::error::AppError::BadRequest(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::CONFLICT\s*,\s*json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\),?\s*\)',
        r'crate::error::AppError::Conflict(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::TOO_MANY_REQUESTS\s*,\s*json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\),?\s*\)',
        r'crate::error::AppError::TooManyRequests(\1.to_string())',
        content
    )
    
    # Also handle the Json(json!(...)) wrapper instead of just json!(...)
    content = re.sub(
        r'\(\s*StatusCode::INTERNAL_SERVER_ERROR\s*,\s*Json\(json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\)\),?\s*\)',
        r'crate::error::AppError::Internal(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::NOT_FOUND\s*,\s*Json\(json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\)\),?\s*\)',
        r'crate::error::AppError::NotFound(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::UNAUTHORIZED\s*,\s*Json\(json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\)\),?\s*\)',
        r'crate::error::AppError::Unauthorized(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::FORBIDDEN\s*,\s*Json\(json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\)\),?\s*\)',
        r'crate::error::AppError::Forbidden(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::BAD_REQUEST\s*,\s*Json\(json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\)\),?\s*\)',
        r'crate::error::AppError::BadRequest(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::CONFLICT\s*,\s*Json\(json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\)\),?\s*\)',
        r'crate::error::AppError::Conflict(\1.to_string())',
        content
    )
    content = re.sub(
        r'\(\s*StatusCode::TOO_MANY_REQUESTS\s*,\s*Json\(json!\(\s*\{\s*"error"\s*:\s*([^}]+)\s*\}\s*\)\),?\s*\)',
        r'crate::error::AppError::TooManyRequests(\1.to_string())',
        content
    )
    
    # Fallbacks for other generic json payloads inside conflict/unauthorized
    content = re.sub(
        r'\(\s*StatusCode::CONFLICT\s*,\s*json!\(\s*\{.*?\}\s*\)\s*\)',
        r'crate::error::AppError::Conflict("Conflict occurred".to_string())',
        content,
        flags=re.DOTALL
    )
    content = re.sub(
        r'\(\s*StatusCode::UNAUTHORIZED\s*,\s*json!\(\s*\{.*?\}\s*\)\s*\)',
        r'crate::error::AppError::Unauthorized("Unauthorized".to_string())',
        content,
        flags=re.DOTALL
    )
    content = re.sub(
        r'\(\s*StatusCode::FORBIDDEN\s*,\s*json!\(\s*\{.*?\}\s*\)\s*\)',
        r'crate::error::AppError::Forbidden("Forbidden".to_string())',
        content,
        flags=re.DOTALL
    )

    with open(filepath, 'w') as f:
        f.write(content)

for f in glob.glob('src/**/*.rs', recursive=True):
    fix_file(f)
