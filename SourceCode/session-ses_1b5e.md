**Output:**
```
task_id: ses_1b480a79cffeOKt80MNEbL2MS7 (for resuming to continue this task if needed)

<task_result>
Now I have a thorough understanding of the codebase. Here is my comprehensive analysis:

---

## LogSmart Backend Improvement Opportunities

---

### 1. ERROR HANDLING

**Finding 1.1: Inconsistent Error Types Across Services**

- **Files**: `src/services/auth_service.rs`, `src/services/log_entry_service.rs`, `src/services/template_service.rs` vs. `src/services/user_service.rs`, `src/services/company_service.rs`
- **Problem**: Most services return `(StatusCode, serde_json::Value)` tuples directly, while `UserService` and `CompanyService` use proper error types (`ServiceError` and `CompanyServiceError`). This inconsistency means:
  - Services using tuples lose type safety -- callers must manually construct JSON responses
  - `CompanyServiceError` (lines 17-23 of `company_service.rs`) demonstrates the better pattern: enum variants with `Display`, mapped to status codes at handler boundaries
  - `AuthService`, `LogEntryService`, and `TemplateService` all inline `(StatusCode, json!(...))` construction dozens of times
- **Improvement**: Introduce a unified `AppError` enum (similar to `CompanyServiceError`) with variants for common cases: `NotFound(String)`, `BadRequest(String)`, `Forbidden(String)`, `Conflict(String)`, `Internal(String)`. Implement `From<AppError> for (StatusCode, Json<Value>)` and `From<AppError> for ServiceError`.
- **Effort**: Medium
- **Risk**: Low (internal refactoring, no API changes)

**Finding 1.2: `anyhow::anyhow!` Used for Structured Errors**

- **Files**: 20 occurrences across `src/db/mod.rs`, `src/db/auth_repo.rs`, `src/db/company_repo.rs`, `src/db/user_repo.rs`, `src/services/oauth_service.rs`, `src/images_db.rs`, `src/exports_db.rs`
- **Problem**: `anyhow` errors are used for domain-level errors (e.g., "Passkey not found", "Oauth account not found for user"). `anyhow` is designed for application-level error chaining, not domain errors. These get converted to generic 500 responses when they should map to specific status codes.
- **Improvement**: Replace `anyhow::anyhow!` with a `DbError` enum in the DB layer. Map specific variants (NotFound, UniqueViolation, etc.) to appropriate HTTP status codes at the service/handler boundary.
- **Effort**: Medium
- **Risk**: Low

**Finding 1.3: Duplicate Error Response Construction in Handlers**

- **Files**: Nearly every handler file
- **Problem**: The pattern `(StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "..." })))` is repeated ~100+ times across handlers. While `utils.rs` provides `err_internal()`, `err_not_found()`, etc., many handlers ignore these helpers and construct responses inline.
- **Examples**:
  - `log_entry_handlers.rs` lines 72-77, 132-137, 223-228, etc. -- all inline JSON error construction
  - `branch_handlers.rs` lines 49-55, 98-105, 157-163, etc.
- **Improvement**: Enforce use of `utils::err_*` helpers consistently. Add a clippy lint or code review checklist.
- **Effort**: Low
- **Risk**: Low

---

### 2. HANDLER PATTERNS

**Finding 2.1: Duplicated Company ID Extraction Pattern**

- **Files**: `log_entry_handlers.rs` (lines 49-52, 379-382, 456-459, 613-616, 733-736, 832-835, 904-907, 980-983, 1027-1030), `template_handlers.rs` (lines 56-59, 99-102, 153-156, 292-295, 331-334, 372-375), `branch_handlers.rs` (lines 37-40, 92-95, 133-136, 231-234)
- **Problem**: The pattern `user.company_id.ok_or((StatusCode::FORBIDDEN, Json(json!({ "error": "User is not associated with a company" }))))?` appears at least 20+ times across handlers.
- **Improvement**: Create a helper method on `UserRecord` like `fn company_id_or_forbidden(&self) -> Result<&str, HandlerError>` or an extractor that guarantees company_id is present.
- **Effort**: Low
- **Risk**: Low

**Finding 2.2: Duplicated LogEntryResponse Construction**

- **Files**: `log_entry_handlers.rs` lines 409-420 (get_log_entry) and lines 487-498 (update_log_entry)
- **Problem**: The `LogEntryResponse` construction block (fetching template, processing layout, computing availability) is duplicated verbatim between `get_log_entry` and `update_log_entry`. Same pattern also appears in `list_company_log_entries` (lines 693-704) and `list_user_log_entries` (lines 787-798).
- **Improvement**: Extract into a function `fn build_log_entry_response(entry: &LogEntry, template: &TemplateDocument) -> LogEntryResponse`.
- **Effort**: Low
- **Risk**: Low

**Finding 2.3: Cookie Construction Duplicated Between Register and Login**

- **Files**: `auth_handlers.rs` lines 195-226 (register) and lines 280-308 (login)
- **Problem**: The cookie construction logic (`ls-token=...; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=...`) is duplicated between `register_company_admin` and `login`. The cookie domain lookup and formatting are identical.
- **Improvement**: Extract to `fn build_auth_cookie(token: &str) -> String` and `fn set_auth_cookie(response: &mut Response, token: &str)`.
- **Effort**: Low
- **Risk**: Low

**Finding 2.4: Profile Picture Admin Lookup Logic Duplicated**

- **Files**: `user_handlers.rs` lines 221-248 (upload) and lines 347-374 (delete)
- **Problem**: The entire block that resolves `target_user_id` and `target_picture_id` based on optional email query parameter, including all the authorization checks (company_manager, branch_manager, logsmart_admin), is duplicated between `upload_profile_picture` and `delete_profile_picture_handler`.
- **Improvement**: Extract to a shared function `async fn resolve_profile_picture_target(state, acting_user, target_email) -> Result<(user_id, picture_id), HandlerError>`.
- **Effort**: Low
- **Risk**: Low

---

### 3. SERVICE LAYER

**Finding 3.1: Services Are Unit Structs (No State)**

- **Files**: `auth_service.rs` line 21, `log_entry_service.rs` line 18, `template_service.rs` line 190, `user_service.rs` line 14, `company_service.rs` line 37, `clock_service.rs`, `invitation_service.rs`, `oauth_service.rs`
- **Problem**: All services are `pub struct XxxService;` (unit structs) with only associated functions. They carry no state. This is effectively a namespace pattern, not a true service pattern. The `AppState` is passed as a parameter to every method.
- **Assessment**: This is actually acceptable for this architecture. Axum's state management handles dependency injection via `State<AppState>`. Converting to stateful services would add complexity without benefit. However, the naming is misleading -- these are more like "operation modules" than services.
- **Improvement**: Consider renaming to `XxxOperations` or `XxxHandler` to better reflect their nature, OR keep as-is but document the architectural rationale. Low priority.
- **Effort**: Low
- **Risk**: Low

**Finding 3.2: `LogEntryService` Methods Are Thin Wrappers**

- **Files**: `log_entry_service.rs` lines 406-436
- **Problem**: `list_due_forms` and `get_user_log_entries` are single-line pass-throughs to `logs_db` functions with only error mapping. The service adds no business logic.
- **Improvement**: Either remove these thin wrappers and call `logs_db` directly from handlers, or consolidate the error mapping into a generic helper.
- **Effort**: Low
- **Risk**: Low

**Finding 3.3: `AuthService::login` Returns Inconsistent Data**

- **File**: `auth_service.rs` lines 137-252
- **Problem**: `AuthService::login` returns `(String, String, UserRole)` where the first String is `serde_json::to_string(&response).unwrap()` (line 248) -- a JSON string that is never used by the caller (`auth_handlers.rs` calls `verify_credentials` instead). This appears to be dead code from an earlier design.
- **Improvement**: Remove the unused `login` method entirely, or fix it if it is intended for future use. The `verify_credentials` method is what handlers actually call.
- **Effort**: Low
- **Risk**: Low

---

### 4. MIDDLEWARE

**Finding 4.1: No Request Logging/Tracing Middleware**

- **File**: `main.rs` lines 368-383
- **Problem**: The only middleware layers are CORS (line 369) and rate limiting (line 352-355). There is no request logging middleware (e.g., `tower-http`'s `TraceLayer`). All logging is done manually inside each handler. This means:
  - No automatic request/response timing
  - No consistent request ID tracking
  - No automatic logging of 4xx/5xx responses
- **Improvement**: Add `tower_http::trace::TraceLayer` for structured request logging with latencies.
- **Effort**: Low
- **Risk**: Low

**Finding 4.2: Rate Limit Middleware Consumes Request Body**

- **File**: `rate_limit.rs` lines 307-312
- **Problem**: The rate limit middleware reads the entire request body into memory (`axum::body::to_bytes(body, usize::MAX)`) to extract email for rate limiting. This:
  - Allocates the full body in memory for every request
  - Uses `usize::MAX` as the limit (no body size protection at middleware level)
  - Only needs the body for login/register paths but reads it for all paths
- **Improvement**: Only consume body for paths that need email extraction (`/auth/login`, `/auth/register`). For other paths, skip body parsing entirely.
- **Effort**: Low
- **Risk**: Low

**Finding 4.3: CORS Origins Hardcoded**

- **File**: `main.rs` lines 362-366
- **Problem**: Allowed CORS origins are hardcoded: `localhost:5173`, `logsmart.app`, `https://logsmart.app`. No environment variable for staging/dev origins.
- **Improvement**: Read allowed origins from an environment variable (e.g., `ALLOWED_ORIGINS` as comma-separated list), with sensible defaults.
- **Effort**: Low
- **Risk**: Low

**Finding 4.4: No Request Body Size Limit**

- **File**: `main.rs`
- **Problem**: There is no `ContentLengthLimit` layer on the router. While individual handlers check file sizes (e.g., 10MB for profile pictures), there is no global protection against oversized request bodies at the middleware level.
- **Improvement**: Add `tower_http::limit::RequestBodyLimitLayer` with a reasonable default (e.g., 10MB).
- **Effort**: Low
- **Risk**: Low

**Finding 4.5: No Compression Middleware**

- **File**: `main.rs`
- **Problem**: No `tower_http::compression::CompressionLayer` is configured. API responses are sent uncompressed.
- **Improvement**: Add compression middleware for gzip/deflate/brotli.
- **Effort**: Low
- **Risk**: Low

---

### 5. DTOs

**Finding 5.1: Missing Input Validation on DTOs**

- **File**: `dto.rs` (entire file)
- **Problem**: No DTOs use validation attributes. All validation is done manually in handlers (e.g., `payload.email.is_empty()` checks). This means:
  - `RegisterRequest.email` has no format validation at the DTO level
  - `AddTemplateRequest.template_name` has no length limits
  - `ReportRunParams` has no validation on date ranges
- **Improvement**: Add a validation library (e.g., `validator` crate with `#[derive(Validate)]`) to DTOs. This centralizes validation rules and reduces handler boilerplate.
- **Effort**: Medium
- **Risk**: Low

**Finding 5.2: `SubmitLogEntryRequest` Is an Empty Struct**

- **File**: `dto.rs` line 597
- **Problem**: `pub struct SubmitLogEntryRequest {}` has no fields. The endpoint takes no body but still expects a JSON POST with an empty object. This is confusing.
- **Improvement**: Either remove the DTO and use `()` as the extractor, or rename to make it clear it's a marker type.
- **Effort**: Low
- **Risk**: Low

**Finding 5.3: Inconsistent `Debug` Derive on DTOs**

- **File**: `dto.rs`
- **Problem**: Some DTOs derive `Debug` (e.g., `RegisterRequest` line 302, `LoginRequest` line 406) while others do not (e.g., `RenameTemplateRequest` line 679, `DeleteTemplateRequest` line 690). `ToSchema` is also inconsistently applied.
- **Improvement**: Apply `#[derive(Debug, Deserialize, ToSchema)]` consistently to all request DTOs.
- **Effort**: Low
- **Risk**: Low

**Finding 5.4: `InvitationDto` Defined After Its `derive_from!` Macro**

- **File**: `dto.rs` lines 187-210
- **Problem**: The `derive_from!(crate::db::Invitation, InvitationDto, ...)` macro invocation at line 187 references `InvitationDto` before the struct is defined at line 201. This works because Rust processes the entire file, but it is confusing to read.
- **Improvement**: Move the struct definition before the macro invocation.
- **Effort**: Low
- **Risk**: Low

---

### 6. TESTS

**Finding 6.1: Stub Tests Provide No Coverage**

- **Files**: `auth_service.rs` lines 13-19, `log_entry_service.rs` lines 10-16, `user_service.rs` lines 7-12, `template_service.rs` line 184-187
- **Problem**: Most service modules have `assert!(true)` placeholder tests. `template_service.rs` has real unit tests for validation, but auth/log_entry/user services have no meaningful tests.
- **Improvement**: Replace stub tests with actual unit tests for service logic. Use the existing test infrastructure in `tests/common/`.
- **Effort**: Medium
- **Risk**: Low

**Finding 6.2: Missing Test Coverage Areas**

- **Files**: `tests/` directory
- **Problem**: Based on the test file list, there are no dedicated tests for:
  - OAuth flow (`oauth_handlers.rs`)
  - Passkey authentication (`passkey_handlers.rs`)
  - LLM integration (`llm_handlers.rs`)
  - Company export/download flow
  - Report run CRUD operations
- **Improvement**: Add integration tests for these uncovered areas, prioritizing security-critical paths (OAuth, passkeys).
- **Effort**: High
- **Risk**: Low

**Finding 6.3: Duplicate Test Setup Code**

- **File**: `tests/common/mod.rs`
- **Problem**: While there is a `setup_test_database` helper, individual test files likely duplicate database seeding, user creation, and authentication setup. Without reading all test files in detail, the pattern of `assert!(true)` stubs suggests test infrastructure is underdeveloped.
- **Improvement**: Build out factory patterns in `tests/common/` for creating test users, companies, templates, and log entries with proper cleanup.
- **Effort**: Medium
- **Risk**: Low

---

### 7. PERFORMANCE

**Finding 7.1: `list_due_forms_today` Has Complex In-Memory Processing**

- **File**: `log_entry_handlers.rs` lines 39-302
- **Problem**: This handler performs extensive in-memory computation:
  - Fetches all templates, then filters in a loop
  - Makes multiple batch MongoDB queries (latest entries, periods with entries, submission checks, draft checks)
  - Uses `HashSet` deduplication and manual accumulation logic
  - The logic for filtering to `max` results is complex and could produce incorrect results if templates are reordered
- **Improvement**: Push more filtering logic into MongoDB queries. Consider adding a MongoDB aggregation pipeline that computes due forms server-side.
- **Effort**: High
- **Risk**: Medium (behavioral changes possible)

**Finding 7.2: `list_company_log_entries` and `list_user_log_entries` Share Identical Response Building**

- **Files**: `log_entry_handlers.rs` lines 652-709 and 762-810
- **Problem**: Both handlers fetch all templates into a `HashMap`, then iterate entries to build responses with identical logic. This is not an N+1 issue (it was correctly batched), but the duplication means any performance improvement needs to be applied twice.
- **Improvement**: Extract the response-building loop into a shared function (also addresses Finding 2.2).
- **Effort**: Low
- **Risk**: Low

**Finding 7.3: User Cache Has No Invalidation Strategy for Cross-User Operations**

- **File**: `main.rs` line 129-132, `middleware.rs` line 32-53
- **Problem**: The user cache (moka, 10K entries, 15min TTL) is invalidated on profile updates, password resets, and company updates. However:
  - When a user's role is changed by an admin, only that user's cache entry is invalidated (correct)
  - When a company is deleted, all user caches for that company must be invalidated -- this is done in `confirm_company_deletion` but relies on fetching all users first
  - No cache invalidation when a branch is deleted (users in that branch may have stale `branch_id` references)
- **Improvement**: Add cache invalidation on branch deletion. Consider using cache tags or a version-based invalidation scheme for company-wide operations.
- **Effort**: Medium
- **Risk**: Medium

**Finding 7.4: Clippy Suggests `.clamp()` Replacement**

- **File**: `src/db/clock_repo.rs` line 115
- **Problem**: `limit.unwrap_or(25).max(1).min(100)` should be `limit.unwrap_or(25).clamp(1, 100)`.
- **Improvement**: Apply clippy's suggestion.
- **Effort**: Low
- **Risk**: Low

---

### 8. CODE QUALITY (Clippy Warnings)

**Finding 8.1: Functions With Too Many Arguments**

- **Files**: `src/db/user_repo.rs:153` (9 args), `src/db/auth_repo.rs:144` (9 args), `src/db/security_repo.rs:15` (8 args), `src/services/user_service.rs:96` (8 args)
- **Problem**: Clippy flags these as exceeding the 7-argument threshold. These functions are harder to call correctly and more prone to argument-order bugs.
- **Improvement**: Group related parameters into builder structs or parameter structs. For example, `create_oauth_user` could take a `CreateOAuthUserParams` struct.
- **Effort**: Medium
- **Risk**: Low

**Finding 8.2: `&PathBuf` Instead of `&Path`**

- **File**: `src/exports_db.rs` line 18
- **Problem**: `fn safe_path(dir: &PathBuf, ...)` should use `&Path`.
- **Improvement**: Change signature to `fn safe_path(dir: &Path, ...)`.
- **Effort**: Low
- **Risk**: Low

**Finding 8.3: Empty Line After Doc Comment**

- **File**: `tests/common/mod.rs` line 28
- **Problem**: Doc comment followed by empty lines before the function it documents.
- **Improvement**: Remove the blank lines between doc comment and function.
- **Effort**: Low
- **Risk**: Low

**Finding 8.4: `unsafe fn load_secret` in `main.rs`**

- **File**: `main.rs` lines 399-416
- **Problem**: The function is marked `unsafe` but the only unsafe operation inside is `std::env::set_var`, which is not actually unsafe in single-threaded contexts. The `unsafe` keyword here is misleading -- it suggests memory safety concerns that do not exist. Furthermore, the function is called from within an `unsafe` block at line 32-34, which is unnecessary.
- **Improvement**: Remove the `unsafe` annotation. `std::env::set_var` is safe to call; the real concern is thread safety during initialization, which is handled by calling it before any threads are spawned.
- **Effort**: Low
- **Risk**: Low

**Finding 8.5: Duplicated `infer_content_type` Function**

- **Files**: `user_handlers.rs` lines 284-294 and `company_service.rs` lines 4-14
- **Problem**: The exact same `infer_content_type` function (PNG/JPEG/WebP magic byte detection) is duplicated in two files.
- **Improvement**: Move to `utils.rs` or a shared `mime_utils.rs` module.
- **Effort**: Low
- **Risk**: Low

**Finding 8.6: Hardcoded Frontend URL in Multiple Places**

- **Files**: `auth_service.rs` line 293 (`https://logsmart.app`), `branch_handlers.rs` line 278 (`https://logsmart.app`)
- **Problem**: The frontend URL is hardcoded in service/handler code instead of being read from environment. The `FRONTEND_URL` env var exists and is used elsewhere (e.g., `company_handlers.rs` line 398).
- **Improvement**: Use `std::env::var("FRONTEND_URL")` consistently for all URL construction.
- **Effort**: Low
- **Risk**: Low

---

### Summary Table

| Area | Finding | Effort | Risk |
|------|---------|--------|------|
| Error Handling | 1.1 Inconsistent error types across services | Medium | Low |
| Error Handling | 1.2 `anyhow` for domain errors | Medium | Low |
| Error Handling | 1.3 Inline error construction vs helpers | Low | Low |
| Handler Patterns | 2.1 Company ID extraction duplicated 20+ times | Low | Low |
| Handler Patterns | 2.2 LogEntryResponse construction duplicated | Low | Low |
| Handler Patterns | 2.3 Cookie construction duplicated | Low | Low |
| Handler Patterns | 2.4 Profile picture target lookup duplicated | Low | Low |
| Service Layer | 3.1 Unit struct services (naming) | Low | Low |
| Service Layer | 3.2 Thin wrapper service methods | Low | Low |
| Service Layer | 3.3 Dead `AuthService::login` method | Low | Low |
| Middleware | 4.1 No request logging middleware | Low | Low |
| Middleware | 4.2 Rate limit consumes body for all paths | Low | Low |
| Middleware | 4.3 Hardcoded CORS origins | Low | Low |
| Middleware | 4.4 No body size limit | Low | Low |
| Middleware | 4.5 No compression | Low | Low |
| DTOs | 5.1 No input validation library | Medium | Low |
| DTOs | 5.2 Empty `SubmitLogEntryRequest` struct | Low | Low |
| DTOs | 5.3 Inconsistent derives | Low | Low |
| DTOs | 5.4 Macro before struct definition | Low | Low |
| Tests | 6.1 Stub tests | Medium | Low |
| Tests | 6.2 Missing coverage areas | High | Low |
| Tests | 6.3 Duplicate test setup | Medium | Low |
| Performance | 7.1 Complex in-memory due forms logic | High | Medium |
| Performance | 7.2 Duplicated response building | Low | Low |
| Performance | 7.3 Cache invalidation gaps | Medium | Medium |
| Performance | 7.4 `.clamp()` clippy fix | Low | Low |
| Code Quality | 8.1 Too many arguments (4 functions) | Medium | Low |
| Code Quality | 8.2 `&PathBuf` vs `&Path` | Low | Low |
| Code Quality | 8.3 Empty line after doc comment | Low | Low |
| Code Quality | 8.4 Misleading `unsafe` annotation | Low | Low |
| Code Quality | 8.5 Duplicated `infer_content_type` | Low | Low |
| Code Quality | 8.6 Hardcoded frontend URL | Low | Low |

**Quick wins** (low effort, low risk): 1.3, 2.1, 2.2, 2.3, 2.4, 3.3, 4.1, 4.2, 4.3, 4.4, 5.2, 5.3, 5.4, 7.2, 7.4, 8.2, 8.3, 8.4, 8.5, 8.6 -- 20 items that can be addressed in focused PRs.
</task_result>
```

---

## Assistant (Plan · qwen3.6-plus-free · 19.2s)

_Thinking:_

The exploration found many improvement opportunities. Let me organize these into a clear, actionable list for the user, grouped by priority and effort.

Here's what I found across the backend:

## Quick Wins (Low effort, Low risk)

| # | Area | What | Impact |
|---|------|------|--------|
| 1 | **Handlers** | Company ID extraction duplicated 20+ times → `user.company_id_or_forbidden()` | -50 lines |
| 2 | **Handlers** | `LogEntryResponse` construction duplicated 4x → extract helper | -40 lines |
| 3 | **Handlers** | Cookie construction duplicated in register/login → extract helper | -20 lines |
| 4 | **Handlers** | Profile picture admin lookup duplicated → extract helper | -30 lines |
| 5 | **Handlers** | Inline `json!({ "error": ... })` 100+ times → enforce `utils::err_*` helpers | Consistency |
| 6 | **Middleware** | No request logging → add `tower-http` `TraceLayer` | Observability |
| 7 | **Middleware** | Rate limit reads body for ALL paths → only for login/register | Performance |
| 8 | **Middleware** | No body size limit → add `RequestBodyLimitLayer` | Security |
| 9 | **Middleware** | No compression → add `CompressionLayer` | Performance |
| 10 | **Middleware** | CORS origins hardcoded → env var | Flexibility |
| 11 | **Services** | Dead `AuthService::login` method → delete | Cleanup |
| 12 | **Code Quality** | Duplicated `infer_content_type` (2 files) → move to `utils.rs` | -15 lines |
| 13 | **Code Quality** | Hardcoded `https://logsmart.app` → use `FRONTEND_URL` env var | Consistency |
| 14 | **Code Quality** | `&PathBuf` → `&Path`, misleading `unsafe`, empty doc comment line | Clippy fixes |
| 15 | **DTOs** | Empty `SubmitLogEntryRequest` → use `()` | Clarity |
| 16 | **DTOs** | Inconsistent `Debug`/`ToSchema` derives → standardize | Consistency |

## Medium Effort

| # | Area | What | Impact |
|---|------|------|--------|
| 17 | **Error Handling** | Unified `AppError` enum (currently 3 services use tuples, 2 use proper enums) | Type safety |
| 18 | **Error Handling** | Replace `anyhow::anyhow!` with `DbError` enum for domain errors | Proper status codes |
| 19 | **DTOs** | Add `validator` crate for DTO-level validation | Less handler boilerplate |
| 20 | **Performance** | Cache invalidation gap on branch deletion | Correctness |
| 21 | **Tests** | Replace `assert!(true)` stub tests with real unit tests | Coverage |

## Higher Effort

| # | Area | What | Impact |
|---|------|------|--------|
| 22 | **Performance** | `list_due_forms_today` complex in-memory logic → push to MongoDB | Performance |
| 23 | **Tests** | Missing coverage: OAuth, passkeys, LLM, exports | Security coverage |
| 24 | **Code Quality** | 4 functions with 8-9 args → parameter structs | Maintainability |

---

**My recommendation**: Tackle items **1-5** (handler dedup) and **6-10** (middleware) first — they're quick, low-risk, and give immediate wins. Then **17-18** (error handling) for architectural improvement.

Which ones do you want to work on?

---

## User

create a plan overview for all tasks then begin

---

## Assistant (Plan · qwen3.6-plus-free · 23.7s)

_Thinking:_

The user wants me to create a comprehensive plan for all the improvements I identified, then begin execution. Let me organize this into a structured plan with clear phases and tasks.

Let me think about the best ordering:

1. **Phase 1: Quick Handler Dedup** - Low risk, high impact, builds momentum
2. **Phase 2: Middleware Improvements** - Independent of handler changes
3. **Phase 3: Error Handling Architecture** - Foundational, affects everything
4. **Phase 4: Code Quality & DTOs** - Cleanup items
5. **Phase 5: Tests & Performance** - Higher effort items

I should create a detailed plan with specific file paths, line numbers, and implementation details for each task. Let me also think about dependencies:
- Error handling (Phase 3) should come before handler dedup (Phase 1) since the dedup work should use the new error types
- Actually, the handler dedup can use existing `utils::err_*` helpers, so it's independent
- Middleware changes are independent
- Code quality items are independent

Let me organize this properly and then start executing.


**Tool: todowrite**

**Input:**
```json
{
  "todos": [
    {
      "content": "Phase 1: Error Handling Architecture - Unified AppError enum + DbError enum",
      "priority": "high",
      "status": "pending"
    },
    {
      "content": "Phase 2: Handler Deduplication - Company ID helper, LogEntryResponse builder, cookie helper, profile picture resolver",
      "priority": "high",
      "status": "pending"
    },
    {
      "content": "Phase 3: Handler Consistency - Enforce utils::err_* helpers everywhere",
      "priority": "high",
      "status": "pending"
    },
    {
      "content": "Phase 4: Middleware - TraceLayer, rate limit body fix, body size limit, compression, CORS env var",
      "priority": "high",
      "status": "pending"
    },
    {
      "content": "Phase 5: Code Quality - infer_content_type dedup, FRONTEND_URL, PathBuf, unsafe, empty DTO, derives",
      "priority": "medium",
      "status": "pending"
    },
    {
      "content": "Phase 6: Services - Delete dead AuthService::login, thin wrapper cleanup",
      "priority": "medium",
      "status": "pending"
    },
    {
      "content": "Phase 7: DTOs - validator crate, SubmitLogEntryRequest fix, macro ordering",
      "priority": "medium",
      "status": "pending"
    },
    {
      "content": "Phase 8: Tests - Replace stub tests, add missing coverage areas",
      "priority": "medium",
      "status": "pending"
    },
    {
      "content": "Phase 9: Performance - Cache invalidation on branch delete, list_due_forms optimization",
      "priority": "low",
      "status": "pending"
    },
    {
      "content": "Final: cargo check + test + clippy verification",
      "priority": "high",
      "status": "pending"
    }
  ]
}
```

**Output:**
```
[
  {
    "content": "Phase 1: Error Handling Architecture - Unified AppError enum + DbError enum",
    "status": "pending",
    "priority": "high"
  },
  {
    "content": "Phase 2: Handler Deduplication - Company ID helper, LogEntryResponse builder, cookie helper, profile picture resolver",
    "status": "pending",
    "priority": "high"
  },
  {
    "content": "Phase 3: Handler Consistency - Enforce utils::err_* helpers everywhere",
    "status": "pending",
    "priority": "high"
  },
  {
    "content": "Phase 4: Middleware - TraceLayer, rate limit body fix, body size limit, compression, CORS env var",
    "status": "pending",
    "priority": "high"
  },
  {
    "content": "Phase 5: Code Quality - infer_content_type dedup, FRONTEND_URL, PathBuf, unsafe, empty DTO, derives",
    "status": "pending",
    "priority": "medium"
  },
  {
    "content": "Phase 6: Services - Delete dead AuthService::login, thin wrapper cleanup",
    "status": "pending",
    "priority": "medium"
  },
  {
    "content": "Phase 7: DTOs - validator crate, SubmitLogEntryRequest fix, macro ordering",
    "status": "pending",
    "priority": "medium"
  },
  {
    "content": "Phase 8: Tests - Replace stub tests, add missing coverage areas",
    "status": "pending",
    "priority": "medium"
  },
  {
    "content": "Phase 9: Performance - Cache invalidation on branch delete, list_due_forms optimization",
    "status": "pending",
    "priority": "low"
  },
  {
    "content": "Final: cargo check + test + clippy verification",
    "status": "pending",
    "priority": "high"
  }
]
```

---

## Assistant (Plan · qwen3.6-plus-free · 50.5s)

_Thinking:_

The user wants a comprehensive plan. Let me create a detailed plan overview with all tasks organized by phase, then begin execution. I need to be thorough but concise.

Let me map out the dependencies:
- Phase 1 (Error Handling) should come first because Phases 2-3 will benefit from the unified error types
- Phase 4 (Middleware) is independent
- Phase 5 (Code Quality) is independent
- Phase 6 (Services) is independent
- Phase 7 (DTOs) is independent
- Phase 8 (Tests) depends on previous phases being stable
- Phase 9 (Performance) is highest risk, should come last

Let me create a detailed plan.

# LogSmart Backend Improvement Plan

## Overview
24 improvements across 9 phases, organized by dependency and risk. Estimated total: ~2,000 lines changed across ~30 files.

---

## Phase 1: Error Handling Architecture (Foundation)
**Why first**: All subsequent handler/service work benefits from unified error types.

| Task | Files | Detail |
|------|-------|--------|
| 1.1 Create `AppError` enum | `src/error.rs` (new) | `NotFound`, `BadRequest`, `Forbidden`, `Conflict`, `Unauthorized`, `Internal` with `Display` + `IntoResponse` |
| 1.2 Create `DbError` enum | `src/db/error.rs` (new) | `NotFound`, `UniqueViolation`, `ForeignKeyViolation`, `Internal` with `From<sqlx::Error>` |
| 1.3 Migrate `CompanyServiceError` | `src/services/company_service.rs` | Replace with `AppError`, delete custom enum |
| 1.4 Migrate `AuthService` | `src/services/auth_service.rs` | Replace `(StatusCode, Json<Value>)` returns with `AppError` |
| 1.5 Migrate `LogEntryService` | `src/services/log_entry_service.rs` | Same |
| 1.6 Migrate `TemplateService` | `src/services/template_service.rs` | Same |
| 1.7 Replace `anyhow` in DB layer | `src/db/*.rs` (8 files) | Replace `anyhow::anyhow!("Not found")` with `DbError::NotFound` |

**Risk**: Low (internal refactoring, no API changes)
**Effort**: Medium

---

## Phase 2: Handler Deduplication
**Depends on**: Phase 1 (uses `AppError` in new helpers)

| Task | Files | Detail |
|------|-------|--------|
| 2.1 Company ID helper | `src/db/types.rs`, all handlers | Add `UserRecord::company_id_or_forbidden()` → replaces 20+ inline `ok_or(...)` patterns |
| 2.2 LogEntryResponse builder | `src/handlers/log_entry_handlers.rs` | Extract `build_log_entry_response()` → eliminates 4x duplication |
| 2.3 Cookie helper | `src/handlers/auth_handlers.rs` | Extract `set_auth_cookie()` → eliminates register/login duplication |
| 2.4 Profile picture resolver | `src/handlers/user_handlers.rs` | Extract `resolve_profile_picture_target()` → eliminates upload/delete duplication |

**Risk**: Low
**Effort**: Low

---

## Phase 3: Handler Error Consistency
**Depends on**: Phase 1 (uses `AppError`), Phase 2 (uses new helpers)

| Task | Files | Detail |
|------|-------|--------|
| 3.1 Replace inline JSON errors | All `src/handlers/*.rs` (10 files) | Replace `Json(json!({ "error": ... }))` with `utils::err_*` or `AppError` |

**Risk**: Low
**Effort**: Medium (~100 replacements across 10 files)

---

## Phase 4: Middleware Improvements
**Independent** of all other phases

| Task | Files | Detail |
|------|-------|--------|
| 4.1 Add `TraceLayer` | `src/main.rs`, `Cargo.toml` | Structured request logging with latency, request IDs |
| 4.2 Rate limit body fix | `src/middleware/rate_limit.rs` | Only consume body for `/auth/login`, `/auth/register` paths |
| 4.3 Body size limit | `src/main.rs` | Add `RequestBodyLimitLayer` (10MB default) |
| 4.4 Compression | `src/main.rs`, `Cargo.toml` | Add `CompressionLayer` (gzip/brotli) |
| 4.5 CORS env var | `src/main.rs` | Read `ALLOWED_ORIGINS` from env, fallback to defaults |

**Risk**: Low
**Effort**: Low

---

## Phase 5: Code Quality Cleanup
**Independent** of all other phases

| Task | Files | Detail |
|------|-------|--------|
| 5.1 Dedup `infer_content_type` | `src/handlers/user_handlers.rs`, `src/services/company_service.rs` → `src/utils.rs` | Move magic byte detection to shared util |
| 5.2 `FRONTEND_URL` env var | `src/services/auth_service.rs`, `src/handlers/branch_handlers.rs` | Replace hardcoded `https://logsmart.app` |
| 5.3 `&PathBuf` → `&Path` | `src/exports_db.rs:18` | Clippy fix |
| 5.4 Remove misleading `unsafe` | `src/main.rs:399-416` | `set_var` isn't actually unsafe in init context |
| 5.5 Empty doc comment line | `tests/common/mod.rs:28` | Clippy fix |
| 5.6 `.clamp()` replacement | `src/db/clock_repo.rs:115` | Clippy fix |
| 5.7 `SubmitLogEntryRequest` → `()` | `src/dto.rs:597`, handler | Delete empty struct, use unit type |
| 5.8 Consistent derives on DTOs | `src/dto.rs` | Add `Debug`, `ToSchema` where missing |
| 5.9 Macro ordering | `src/dto.rs:187-210` | Move struct before `derive_from!` macro |

**Risk**: Low
**Effort**: Low

---

## Phase 6: Service Layer Cleanup
**Independent** of all other phases

| Task | Files | Detail |
|------|-------|--------|
| 6.1 Delete dead `AuthService::login` | `src/services/auth_service.rs:137-252` | Returns unused JSON string, callers use `verify_credentials` |
| 6.2 Thin wrapper cleanup | `src/services/log_entry_service.rs` | Remove pass-through methods, call `logs_db` directly from handlers |

**Risk**: Low
**Effort**: Low

---

## Phase 7: DTO Improvements
**Independent** of all other phases

| Task | Files | Detail |
|------|-------|--------|
| 7.1 Add `validator` crate | `Cargo.toml`, `src/dto.rs` | `#[derive(Validate)]` on request DTOs, move validation from handlers |
| 7.2 Validate in handlers | All handlers | Replace manual `payload.email.is_empty()` checks with `payload.validate()` |

**Risk**: Low (validation logic preserved, just relocated)
**Effort**: Medium

---

## Phase 8: Test Improvements
**Depends on**: Phases 1-7 (code must be stable first)

| Task | Files | Detail |
|------|-------|--------|
| 8.1 Replace stub tests | `src/services/auth_service.rs`, `log_entry_service.rs`, `user_service.rs` | Real unit tests using `tests/common/` infrastructure |
| 8.2 Add OAuth test coverage | `tests/oauth_tests.rs` (new) | OAuth flow, token exchange, user creation |
| 8.3 Add passkey test coverage | `tests/passkey_tests.rs` (new) | Passkey registration, authentication, session management |
| 8.4 Add export test coverage | `tests/export_tests.rs` (new) | Company export, download, deletion flow |

**Risk**: Low
**Effort**: High

---

## Phase 9: Performance
**Highest risk, comes last**

| Task | Files | Detail |
|------|-------|--------|
| 9.1 Cache invalidation on branch delete | `src/main.rs` (cache), `src/db/branch_repo.rs` | Invalidate user cache entries when branch is deleted |
| 9.2 `list_due_forms_today` optimization | `src/handlers/log_entry_handlers.rs` | Push filtering into MongoDB aggregation pipeline |

**Risk**: Medium (behavioral changes possible)
**Effort**: High

---

## Execution Strategy

- **Phases 1-5**: Parallel agents where possible, sequential verification
- **Phases 6-7**: Sequential (DTO changes affect handler signatures)
- **Phases 8-9**: Sequential, after all code is stable
- **Verification**: `cargo check` + `cargo test` + `cargo clippy` after each phase

---
