# Phase 7 & 8: DTO Validation & Test Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add validator crate for DTO validation, replace manual validation in handlers with declarative validation attributes, and replace stub tests with real test cases covering valid input, invalid input, error handling, and edge cases.

**Architecture:** 
- Add `validator` crate as a dependency with derive feature
- Apply validation attributes to all request DTOs in `src/dto.rs` (email fields, required strings, name field lengths, numeric ranges)
- Update handlers to call `payload.validate()?` instead of manual field checks
- Replace stub tests (assert!(true)) in service files with real test cases that validate behavior
- Maintain test isolation and clarity through focused test cases

**Tech Stack:** 
- Rust/Axum backend
- `validator` crate with derive macros
- `tokio::test` for async tests
- Existing test helpers from `tests/common/`

---

## File Structure

**Files to modify:**
- `back-end/Cargo.toml` - Add validator dependency
- `back-end/src/dto.rs` - Add Validate derive and validation rules to request DTOs
- `back-end/src/handlers/*.rs` - Replace manual validation with `payload.validate()?`
- `back-end/src/services/auth_service.rs` - Replace stub test with real tests
- `back-end/src/services/log_entry_service.rs` - Replace stub tests with real tests
- `back-end/src/services/user_service.rs` - Replace stub tests with real tests
- `back-end/tests/services/auth_service_tests.rs` - Add real test cases
- `back-end/tests/services/log_entry_service_tests.rs` - Add real test cases
- `back-end/tests/services/user_service_tests.rs` - Add real test cases

---

## Phase 7: DTO Validation

### Task 1: Add validator crate dependency

**Files:**
- Modify: `back-end/Cargo.toml:15-55`

- [ ] **Step 1: Add validator to dependencies**

Edit `Cargo.toml` and add the validator crate to the dependencies section after the existing crates:

```toml
validator = { version = "0.18", features = ["derive"] }
```

The final dependencies section (lines 15-55) should include:
```toml
[dependencies]
anyhow = "1.0"
axum = { version = "0.8.7", features = ["macros"] }
base64 = "0.22"
axum-extra = { version = "0.12.2", features = ["typed-header"] }
async-trait = "0.1"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio-native-tls", "macros", "chrono"] }
tokio = { version = "1", features = ["rt-multi-thread"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rand = "0.10.0"
jsonwebtoken = { version = "10", features = ["rust_crypto"] }
chrono = { version = "0.4", features = ["serde"] }
argon2 = "0.5"
regex = "1.10"
once_cell = "1.19"
tower = { version = "0.5", features = ["util"] }
tower-http = { version = "0.6", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
uuid = { version = "1.0", features = ["v4", "v6", "serde"] }
headers = "0.4.1"
governor = "0.10.4"
dashmap = "6.1"
utoipa = { version = "5", features = ["axum_extras", "chrono", "uuid"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
lettre = { version = "0.11", features = ["tokio1-native-tls", "smtp-transport", "builder"] }
dotenvy = "0.15"
mongodb = "3.4.1"
futures-util = "0.3.31"
schemars = "1.1.0"
webauthn-rs = { version = "0.5", features = ["danger-allow-state-serialisation", "conditional-ui"] }
url = "2.5.7"
percent-encoding = "2.3"
webauthn-rs-proto = "0.5.4"
openidconnect = { version = "4.0.1", features = [ "reqwest" ] }
reqwest = { version = "0.13.1", features = ["json"] }
moka = { version = "0.12.13", features = ["future"] }
image = "0.25"
zip = { version = "4", default-features = false, features = ["deflate"] }
validator = { version = "0.18", features = ["derive"] }
```

- [ ] **Step 2: Run cargo check to verify dependency loads**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS with no errors

- [ ] **Step 3: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add Cargo.toml
git commit -m "Back-End: Add validator crate dependency with derive feature"
```

---

### Task 2: Add validation to RegisterRequest DTO

**Files:**
- Modify: `back-end/src/dto.rs:302-316`
- Modify: `back-end/src/dto.rs:1-6` (add validator import)

- [ ] **Step 1: Add validator import to dto.rs**

At the top of `src/dto.rs`, after existing imports, add:

```rust
use validator::Validate;
```

The imports should look like:
```rust
use crate::{
    db::{self, UserRole},
    logs_db,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;
```

- [ ] **Step 2: Add Validate derive and validation attributes to RegisterRequest**

Modify the `RegisterRequest` struct at lines 302-316:

```rust
#[derive(Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct RegisterRequest {
    #[schema(example = "admin@example.com")]
    #[validate(email)]
    pub email: String,
    #[schema(example = "John")]
    #[validate(length(min = 1, max = 255))]
    pub first_name: String,
    #[schema(example = "Doe")]
    #[validate(length(min = 1, max = 255))]
    pub last_name: String,
    #[schema(example = "SecurePass123!")]
    #[validate(length(min = 1))]
    pub password: String,
    #[schema(example = "Example Corp")]
    #[validate(length(min = 1, max = 255))]
    pub company_name: String,
    #[schema(example = "123 Main St, City, Country")]
    #[validate(length(min = 1, max = 255))]
    pub company_address: String,
}
```

- [ ] **Step 3: Verify no compilation errors**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS - RegisterRequest now has validation attributes

- [ ] **Step 4: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/dto.rs
git commit -m "Back-End: Add validation attributes to RegisterRequest DTO"
```

---

### Task 3: Add validation to LoginRequest DTO

**Files:**
- Modify: `back-end/src/dto.rs` (find LoginRequest)

- [ ] **Step 1: Find LoginRequest in dto.rs**

Search for `struct LoginRequest` to locate the struct definition.

- [ ] **Step 2: Add Validate derive and validation attributes to LoginRequest**

Modify the struct to include the `Validate` derive and add validation rules:

```rust
#[derive(Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct LoginRequest {
    #[schema(example = "user@example.com")]
    #[validate(email)]
    pub email: String,
    #[schema(example = "SecurePass123!")]
    #[validate(length(min = 1))]
    pub password: String,
}
```

- [ ] **Step 3: Verify no compilation errors**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/dto.rs
git commit -m "Back-End: Add validation attributes to LoginRequest DTO"
```

---

### Task 4: Add validation to UpdateProfileRequest DTO

**Files:**
- Modify: `back-end/src/dto.rs` (find UpdateProfileRequest)

- [ ] **Step 1: Find UpdateProfileRequest struct**

Search for `struct UpdateProfileRequest` in dto.rs

- [ ] **Step 2: Add Validate derive and validation attributes**

```rust
#[derive(Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct UpdateProfileRequest {
    #[validate(length(min = 1, max = 255))]
    pub first_name: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub last_name: Option<String>,
}
```

- [ ] **Step 3: Verify compilation**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/dto.rs
git commit -m "Back-End: Add validation attributes to UpdateProfileRequest DTO"
```

---

### Task 5: Add validation to other critical request DTOs

**Files:**
- Modify: `back-end/src/dto.rs` (multiple request DTOs)

Request DTOs to validate (find and update each):
1. `CreateBranchRequest` - lines 115-121
2. `UpdateBranchRequest` - lines 123-131
3. `AdminUpdateMemberRequest` - lines 21-35
4. `RemoveMemberRequest` - lines 48-52
5. `CancelInvitationRequest` - lines 42-46
6. `RequestPasswordResetRequest` - find it
7. `ResetPasswordRequest` - find it
8. `AddTemplateRequest` - lines 337-347
9. `UpdateTemplateRequest` - lines 354-366

- [ ] **Step 1: Update CreateBranchRequest**

```rust
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateBranchRequest {
    #[schema(example = "London Office")]
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[schema(example = "123 Regent St, London")]
    #[validate(length(min = 1, max = 255))]
    pub address: String,
}
```

- [ ] **Step 2: Update UpdateBranchRequest**

```rust
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct UpdateBranchRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    #[validate(length(min = 1))]
    pub branch_id: String,
    #[schema(example = "London Office")]
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[schema(example = "123 Regent St, London")]
    #[validate(length(min = 1, max = 255))]
    pub address: String,
}
```

- [ ] **Step 3: Update AdminUpdateMemberRequest**

```rust
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct AdminUpdateMemberRequest {
    #[schema(example = "user@example.com")]
    #[validate(email)]
    pub email: String,
    #[schema(example = "Jane")]
    #[validate(length(min = 1, max = 255))]
    pub first_name: String,
    #[schema(example = "Smith")]
    #[validate(length(min = 1, max = 255))]
    pub last_name: String,
    #[schema(example = "staff")]
    #[validate(length(min = 1))]
    pub role: String,
    #[schema(example = "branch-uuid-here")]
    pub branch_id: Option<String>,
    #[schema(example = "uuid-of-profile-picture")]
    pub profile_picture_id: Option<String>,
}
```

- [ ] **Step 4: Update RemoveMemberRequest**

```rust
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct RemoveMemberRequest {
    #[schema(example = "user@example.com")]
    #[validate(email)]
    pub email: String,
}
```

- [ ] **Step 5: Update CancelInvitationRequest**

```rust
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CancelInvitationRequest {
    #[schema(example = "invitation-uuid-here")]
    #[validate(length(min = 1))]
    pub invitation_id: String,
}
```

- [ ] **Step 6: Find and update RequestPasswordResetRequest**

Search for this struct and update to:

```rust
#[derive(Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct RequestPasswordResetRequest {
    #[schema(example = "user@example.com")]
    #[validate(email)]
    pub email: String,
}
```

- [ ] **Step 7: Find and update ResetPasswordRequest**

Search for this struct and update to:

```rust
#[derive(Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct ResetPasswordRequest {
    #[schema(example = "reset-token-here")]
    #[validate(length(min = 1))]
    pub token: String,
    #[schema(example = "NewSecurePass123!")]
    #[validate(length(min = 1))]
    pub new_password: String,
}
```

- [ ] **Step 8: Update AddTemplateRequest**

```rust
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct AddTemplateRequest {
    #[schema(example = "Kitchen Daily Log")]
    #[validate(length(min = 1, max = 255))]
    pub template_name: String,
    #[schema(example = "[\"field1\", \"field2\"]")]
    pub template_layout: logs_db::TemplateLayout,
    #[schema(example = "{\"frequency\": \"daily\", \"time\": \"08:00\"}")]
    pub schedule: logs_db::Schedule,
    #[schema(example = "branch-uuid-here")]
    pub branch_id: Option<String>,
}
```

- [ ] **Step 9: Update UpdateTemplateRequest**

```rust
#[derive(Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct UpdateTemplateRequest {
    #[schema(example = "Kitchen Daily Log")]
    #[validate(length(min = 1, max = 255))]
    pub template_name: String,
    #[schema(example = "[\"field1\", \"field2\"]")]
    pub template_layout: Option<logs_db::TemplateLayout>,
    #[schema(example = "{\"frequency\": \"daily\", \"time\": \"08:00\"}")]
    pub schedule: Option<logs_db::Schedule>,
    #[schema(example = "Major Update")]
    #[validate(length(max = 255))]
    pub version_name: Option<String>,
    #[schema(example = "branch-uuid-here")]
    pub branch_id: Option<String>,
}
```

- [ ] **Step 10: Verify compilation**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS - All DTOs now have validation

- [ ] **Step 11: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/dto.rs
git commit -m "Back-End: Add validation attributes to all critical request DTOs"
```

---

### Task 6: Update auth_handlers.rs to use DTO validation

**Files:**
- Modify: `back-end/src/handlers/auth_handlers.rs:132-180`

- [ ] **Step 1: Find register_company_admin handler**

Look at lines 132-180 where manual validation occurs:

```rust
if payload.email.is_empty()
    || payload.first_name.is_empty()
    || payload.last_name.is_empty()
    || payload.password.is_empty()
    || payload.company_name.is_empty()
    || payload.company_address.is_empty()
{
    return Err(err_bad_request("All required fields must be provided"));
}

if !validate_email(&payload.email) {
    return Err(err_bad_request("Invalid email format"));
}
```

- [ ] **Step 2: Replace with payload.validate() call**

Replace those manual validation checks with:

```rust
payload.validate().map_err(|e| {
    err_bad_request(&format!("Validation failed: {}", e))
})?;
```

- [ ] **Step 3: Check for other handlers with manual validation**

Search for other handlers in `auth_handlers.rs` that do manual validation (look for `.is_empty()` checks). Examples:
- `login` handler
- `request_password_reset` handler  
- `reset_password` handler
- `update_profile` handler

- [ ] **Step 4: Update each handler with payload.validate()?**

For each handler that currently does manual validation, replace with:

```rust
payload.validate().map_err(|e| {
    err_bad_request(&format!("Validation failed: {}", e))
})?;
```

- [ ] **Step 5: Verify compilation**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/handlers/auth_handlers.rs
git commit -m "Back-End: Replace manual validation with DTO validation in auth handlers"
```

---

### Task 7: Update other handlers to use DTO validation

**Files:**
- Modify: `back-end/src/handlers/branch_handlers.rs`
- Modify: `back-end/src/handlers/company_handlers.rs`
- Modify: `back-end/src/handlers/template_handlers.rs`
- Modify: `back-end/src/handlers/user_handlers.rs`
- Modify: `back-end/src/handlers/invitation_handlers.rs`

- [ ] **Step 1: Update branch_handlers.rs**

Find handlers that accept branch request DTOs (CreateBranchRequest, UpdateBranchRequest, etc.) and replace manual validation with `payload.validate()?`

- [ ] **Step 2: Update company_handlers.rs**

Same process - replace manual validation with `payload.validate()?`

- [ ] **Step 3: Update template_handlers.rs**

Find handlers for AddTemplateRequest, UpdateTemplateRequest and replace manual validation

- [ ] **Step 4: Update user_handlers.rs**

Replace manual validation in user-related handlers

- [ ] **Step 5: Update invitation_handlers.rs**

Replace manual validation in invitation handlers

- [ ] **Step 6: Verify all handlers compile**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/handlers/
git commit -m "Back-End: Replace manual validation with DTO validation across all handlers"
```

---

## Phase 8: Test Coverage

### Task 8: Replace stub tests in auth_service.rs

**Files:**
- Modify: `back-end/src/services/auth_service.rs:11-18`

- [ ] **Step 1: Remove stub test from auth_service.rs**

Replace the stub test at lines 11-18:

```rust
#[cfg(test)]
mod auth_service_tests {
    #[tokio::test]
    async fn test_auth_service_basic() {
        // Basic test to ensure service compiles
        assert!(true);
    }
}
```

With this note at the top of the module:

```rust
#[cfg(test)]
mod tests {
    // Tests moved to tests/services/auth_service_tests.rs
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/services/auth_service.rs
git commit -m "Back-End: Remove stub test from auth_service.rs"
```

---

### Task 9: Add real tests to auth_service_tests.rs

**Files:**
- Modify: `back-end/tests/services/auth_service_tests.rs`

- [ ] **Step 1: Add test for register_admin success case**

Add this test to the file:

```rust
#[tokio::test]
async fn test_register_admin_success() {
    let pool = setup_test_db().await;
    
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("admin{}@example.com", unique_id);
    
    // Call register_admin
    let result = AuthService::register_admin(
        &pool,
        &email,
        "John",
        "Doe",
        "SecurePass123!",
        "Test Company",
        "123 Main St",
        None,
        None,
    ).await;
    
    assert!(result.is_ok());
    let (user, token) = result.unwrap();
    assert_eq!(user.email, email);
    assert_eq!(user.first_name, "John");
    assert_eq!(user.last_name, "Doe");
    assert!(!token.is_empty());
}
```

- [ ] **Step 2: Add test for register_admin with invalid email**

```rust
#[tokio::test]
async fn test_register_admin_invalid_email() {
    let pool = setup_test_db().await;
    
    // Call with invalid email format
    let result = AuthService::register_admin(
        &pool,
        "invalid-email",
        "John",
        "Doe",
        "SecurePass123!",
        "Test Company",
        "123 Main St",
        None,
        None,
    ).await;
    
    // Should fail due to invalid email
    assert!(result.is_err());
}
```

- [ ] **Step 3: Add test for register_admin with weak password**

```rust
#[tokio::test]
async fn test_register_admin_weak_password() {
    let pool = setup_test_db().await;
    
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    
    // Call with weak password (less than 8 chars)
    let result = AuthService::register_admin(
        &pool,
        &format!("admin{}@example.com", unique_id),
        "John",
        "Doe",
        "weak",  // Too weak
        "Test Company",
        "123 Main St",
        None,
        None,
    ).await;
    
    // Should fail due to password policy
    assert!(result.is_err());
}
```

- [ ] **Step 4: Add test for request_password_reset success**

```rust
#[tokio::test]
async fn test_request_password_reset_success() {
    let pool = setup_test_db().await;
    
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("test{}@example.com", unique_id);
    
    // Create user first
    create_test_user(&pool, &email, Some("company123")).await;
    
    // Request password reset
    let result = AuthService::request_password_reset(&pool, &email, None, None).await;
    
    assert!(result.is_ok());
}
```

- [ ] **Step 5: Add test for request_password_reset non-existent email**

```rust
#[tokio::test]
async fn test_request_password_reset_nonexistent_email() {
    let pool = setup_test_db().await;
    
    // Request password reset for non-existent email
    let result = AuthService::request_password_reset(
        &pool,
        "nonexistent@example.com",
        None,
        None
    ).await;
    
    // Should not error (security: don't reveal if email exists)
    assert!(result.is_ok());
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo test --test auth_service_tests`

Expected: PASS - All new tests pass

- [ ] **Step 7: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add tests/services/auth_service_tests.rs
git commit -m "Back-End: Add real test cases for auth_service"
```

---

### Task 10: Replace stub tests in log_entry_service.rs and add real tests

**Files:**
- Modify: `back-end/src/services/log_entry_service.rs` (remove stubs)
- Modify: `back-end/tests/services/log_entry_service_tests.rs` (add real tests)

- [ ] **Step 1: Find and remove stub tests from log_entry_service.rs**

Search for `#[cfg(test)]` blocks with `assert!(true)` and replace with empty test module or remove entirely.

- [ ] **Step 2: Add test for successful log entry creation**

Add to `tests/services/log_entry_service_tests.rs`:

```rust
#[tokio::test]
async fn test_create_log_entry_success() {
    let pool = setup_test_db().await;
    
    let user = create_test_user(&pool, "user@example.com", Some("company123")).await;
    let template = create_test_template(&pool, &user.company_id.unwrap()).await;
    
    // Create log entry with valid data
    let result = LogEntryService::create_entry(
        &pool,
        &user.id,
        &template.id,
        serde_json::json!({"field1": "value1"}),
    ).await;
    
    assert!(result.is_ok());
    let entry = result.unwrap();
    assert_eq!(entry.user_id, user.id);
    assert_eq!(entry.template_id, template.id);
}
```

- [ ] **Step 3: Add test for invalid user ID**

```rust
#[tokio::test]
async fn test_create_log_entry_invalid_user() {
    let pool = setup_test_db().await;
    
    let template = create_test_template(&pool, "company123").await;
    let invalid_user_id = "invalid-user-id";
    
    // Try to create entry with non-existent user
    let result = LogEntryService::create_entry(
        &pool,
        invalid_user_id,
        &template.id,
        serde_json::json!({}),
    ).await;
    
    assert!(result.is_err());
}
```

- [ ] **Step 4: Add test for invalid template ID**

```rust
#[tokio::test]
async fn test_create_log_entry_invalid_template() {
    let pool = setup_test_db().await;
    
    let user = create_test_user(&pool, "user@example.com", Some("company123")).await;
    let invalid_template_id = "invalid-template-id";
    
    // Try to create entry with non-existent template
    let result = LogEntryService::create_entry(
        &pool,
        &user.id,
        invalid_template_id,
        serde_json::json!({}),
    ).await;
    
    assert!(result.is_err());
}
```

- [ ] **Step 5: Add test for retrieving log entry**

```rust
#[tokio::test]
async fn test_get_log_entry_success() {
    let pool = setup_test_db().await;
    
    let user = create_test_user(&pool, "user@example.com", Some("company123")).await;
    let template = create_test_template(&pool, &user.company_id.unwrap()).await;
    
    // Create entry first
    let created_entry = LogEntryService::create_entry(
        &pool,
        &user.id,
        &template.id,
        serde_json::json!({"field1": "value1"}),
    ).await.unwrap();
    
    // Retrieve entry
    let result = LogEntryService::get_entry(&pool, &created_entry.id).await;
    
    assert!(result.is_ok());
    let entry = result.unwrap();
    assert_eq!(entry.id, created_entry.id);
}
```

- [ ] **Step 6: Run tests**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo test --test log_entry_service_tests`

Expected: PASS - All new tests pass

- [ ] **Step 7: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/services/log_entry_service.rs tests/services/log_entry_service_tests.rs
git commit -m "Back-End: Replace stubs and add real test cases for log_entry_service"
```

---

### Task 11: Replace stub tests in user_service.rs and add real tests

**Files:**
- Modify: `back-end/src/services/user_service.rs` (remove stubs)
- Modify: `back-end/tests/services/user_service_tests.rs` (add real tests)

- [ ] **Step 1: Remove stub tests from user_service.rs**

Find and remove `#[cfg(test)]` blocks with `assert!(true)`.

- [ ] **Step 2: Add test for successful user creation**

Add to `tests/services/user_service_tests.rs`:

```rust
#[tokio::test]
async fn test_create_user_success() {
    let pool = setup_test_db().await;
    
    let company = create_test_company(&pool).await;
    
    let result = UserService::create_user(
        &pool,
        "newuser@example.com",
        "John",
        "Doe",
        Some("password123"),
        Some(&company.id),
        UserRole::TeamMember,
    ).await;
    
    assert!(result.is_ok());
    let user = result.unwrap();
    assert_eq!(user.email, "newuser@example.com");
    assert_eq!(user.first_name, "John");
}
```

- [ ] **Step 3: Add test for duplicate email**

```rust
#[tokio::test]
async fn test_create_user_duplicate_email() {
    let pool = setup_test_db().await;
    
    let company = create_test_company(&pool).await;
    
    // Create first user
    UserService::create_user(
        &pool,
        "user@example.com",
        "John",
        "Doe",
        Some("password123"),
        Some(&company.id),
        UserRole::TeamMember,
    ).await.unwrap();
    
    // Try to create duplicate
    let result = UserService::create_user(
        &pool,
        "user@example.com",
        "Jane",
        "Smith",
        Some("password123"),
        Some(&company.id),
        UserRole::TeamMember,
    ).await;
    
    assert!(result.is_err());
}
```

- [ ] **Step 4: Add test for updating user**

```rust
#[tokio::test]
async fn test_update_user_success() {
    let pool = setup_test_db().await;
    
    let company = create_test_company(&pool).await;
    let user = create_test_user(&pool, "user@example.com", Some(&company.id)).await;
    
    let result = UserService::update_user(
        &pool,
        &user.id,
        Some("Jane"),
        Some("Smith"),
    ).await;
    
    assert!(result.is_ok());
    let updated = result.unwrap();
    assert_eq!(updated.first_name, "Jane");
    assert_eq!(updated.last_name, "Smith");
}
```

- [ ] **Step 5: Add test for getting user by email**

```rust
#[tokio::test]
async fn test_get_user_by_email_success() {
    let pool = setup_test_db().await;
    
    let company = create_test_company(&pool).await;
    let user = create_test_user(&pool, "user@example.com", Some(&company.id)).await;
    
    let result = UserService::get_user_by_email(&pool, "user@example.com").await;
    
    assert!(result.is_ok());
    let retrieved = result.unwrap().unwrap();
    assert_eq!(retrieved.id, user.id);
}
```

- [ ] **Step 6: Add test for non-existent email**

```rust
#[tokio::test]
async fn test_get_user_by_email_not_found() {
    let pool = setup_test_db().await;
    
    let result = UserService::get_user_by_email(&pool, "nonexistent@example.com").await;
    
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}
```

- [ ] **Step 7: Run tests**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo test --test user_service_tests`

Expected: PASS - All new tests pass

- [ ] **Step 8: Commit**

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add src/services/user_service.rs tests/services/user_service_tests.rs
git commit -m "Back-End: Replace stubs and add real test cases for user_service"
```

---

### Task 12: Verify all tests pass

**Files:**
- Test files: all modified tests

- [ ] **Step 1: Run all unit tests**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo test --lib 2>&1 | tail -50`

Expected: All unit tests pass

- [ ] **Step 2: Run all service tests**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo test --test "*service*" 2>&1 | tail -50`

Expected: All service tests pass

- [ ] **Step 3: Run full test suite**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo test 2>&1 | tail -100`

Expected: All tests pass (target: 59+ tests passing)

- [ ] **Step 4: Run cargo check for final verification**

Run: `cd /mdata/NS/Projects/LogSmart/SourceCode/back-end && cargo check`

Expected: PASS - No compilation errors

- [ ] **Step 5: Create final commit if needed**

If any changes were made:

```bash
cd /mdata/NS/Projects/LogSmart/SourceCode/back-end
git add .
git commit -m "Back-End: Final verification of tests and compilation"
```

---

## Success Criteria

- ✅ `validator` crate added to Cargo.toml with derive feature
- ✅ All request DTOs have `#[derive(Validate)]` with appropriate validation rules
- ✅ Email fields have `#[validate(email)]`
- ✅ Required string fields have `#[validate(length(min = 1))]`
- ✅ Name fields have max length validation
- ✅ Handlers use `payload.validate()?` instead of manual validation
- ✅ Stub tests (assert!(true)) replaced with real test cases
- ✅ Test cases cover: valid input, invalid input, error handling, edge cases
- ✅ `cargo check` passes
- ✅ All unit tests pass (59+)
- ✅ All commits follow convention: "Back-End: [description]"
