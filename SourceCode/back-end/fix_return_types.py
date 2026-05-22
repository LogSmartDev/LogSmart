import os
import re

handlers_dir = 'src/handlers'

for filename in os.listdir(handlers_dir):
    if filename.endswith('.rs'):
        filepath = os.path.join(handlers_dir, filename)
        with open(filepath, 'r') as f:
            content = f.read()
        
        # Replace the return types
        content = re.sub(r'\(StatusCode,\s*(axum::)?Json<serde_json::Value>\)', 'crate::error::AppError', content)
        content = re.sub(r'crate::utils::HandlerError', 'crate::error::AppError', content)
        content = re.sub(r'HandlerError', 'crate::error::AppError', content)
        
        with open(filepath, 'w') as f:
            f.write(content)

print("Replaced return types.")
