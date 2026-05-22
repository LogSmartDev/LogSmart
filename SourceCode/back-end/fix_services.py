import os
import re

def process_file(path):
    with open(path, 'r') as f:
        content = f.read()

    # Replace ServiceError with AppError
    content = content.replace('ServiceError', 'crate::error::AppError')
    
    # Remove imports of svc_err_*
    content = re.sub(r'use crate::utils::\{.*?svc_err.*?\};\n', '', content, flags=re.DOTALL)
    content = re.sub(r'use crate::utils::svc_err_.*?;', '', content)
    
    # Replace svc_err_*(msg)
    # The msg can be a string literal, possibly followed by a comma
    content = re.sub(r'svc_err_internal\(\s*(".*?")\s*,?\s*\)', r'crate::error::AppError::Internal(\1.to_string())', content, flags=re.DOTALL)
    content = re.sub(r'svc_err_not_found\(\s*(".*?")\s*,?\s*\)', r'crate::error::AppError::NotFound(\1.to_string())', content, flags=re.DOTALL)
    content = re.sub(r'svc_err_forbidden\(\s*(".*?")\s*,?\s*\)', r'crate::error::AppError::Forbidden(\1.to_string())', content, flags=re.DOTALL)
    content = re.sub(r'svc_err_bad_request\(\s*(".*?")\s*,?\s*\)', r'crate::error::AppError::BadRequest(\1.to_string())', content, flags=re.DOTALL)
    
    with open(path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('src/services'):
    for f in files:
        if f.endswith('.rs'):
            process_file(os.path.join(root, f))
