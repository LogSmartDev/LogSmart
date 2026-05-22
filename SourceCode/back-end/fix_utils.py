import re
with open('src/utils.rs', 'r') as f:
    content = f.read()

# Fix try_db to use crate::error::AppError
content = content.replace('$crate::utils::svc_err_internal($context)', 'crate::error::AppError::Internal($context.to_string())')

# Remove ServiceError definitions
content = re.sub(r'pub type ServiceError = .*?;\n', '', content)

def remove_fn(name, content):
    pattern = r'pub fn ' + name + r'[\s\S]*?\}\n'
    return re.sub(pattern, '', content)

content = remove_fn('svc_err', content)
content = remove_fn('svc_err_internal', content)
content = remove_fn('svc_err_not_found', content)
content = remove_fn('svc_err_forbidden', content)
content = remove_fn('svc_err_bad_request', content)

with open('src/utils.rs', 'w') as f:
    f.write(content)
