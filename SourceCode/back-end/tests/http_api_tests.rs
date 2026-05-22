use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{ConnectInfo, State},
    http::{HeaderMap, Request, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use back_end::{AppState, db, dto, handlers, middleware::BranchManagerUser};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::sleep;
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;
use webauthn_rs::WebauthnBuilder;

async fn test_register_handler(
    State(state): State<AppState>,
    Json(payload): Json<dto::RegisterRequest>,
) -> axum::response::Response {
    let mock_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let headers = HeaderMap::new();
    let result = handlers::register_company_admin(
        State(state),
        ConnectInfo(mock_addr),
        headers,
        Json(payload),
    )
    .await;
    match result {
        Ok(ok) => ok.into_response(),
        Err(err) => err.into_response(),
    }
}

async fn test_login_handler(
    State(state): State<AppState>,
    Json(payload): Json<dto::LoginRequest>,
) -> axum::response::Response {
    let mock_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let headers = HeaderMap::new();
    let result =
        handlers::login(State(state), ConnectInfo(mock_addr), headers, Json(payload)).await;
    match result {
        Ok(ok) => ok.into_response(),
        Err(err) => err.into_response(),
    }
}

async fn test_invite_handler(
    BranchManagerUser(claims, user): BranchManagerUser,
    State(state): State<AppState>,
    Json(payload): Json<dto::InviteUserRequest>,
) -> Result<(StatusCode, Json<dto::InvitationResponse>), back_end::error::AppError> {
    let mock_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let headers = HeaderMap::new();
    handlers::invite_user(
        BranchManagerUser(claims, user),
        State(state),
        ConnectInfo(mock_addr),
        headers,
        Json(payload),
    )
    .await
}

async fn test_accept_invitation_handler(
    State(state): State<AppState>,
    Json(payload): Json<dto::AcceptInvitationRequest>,
) -> axum::response::Response {
    let mock_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let headers = HeaderMap::new();
    let result =
        handlers::accept_invitation(State(state), ConnectInfo(mock_addr), headers, Json(payload))
            .await;
    match result {
        Ok(ok) => ok.into_response(),
        Err(err) => err.into_response(),
    }
}
async fn get_test_db_pool() -> PgPool {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://admin:adminpassword@localhost:5432/logsmartdb".to_string());

    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test db")
}

async fn setup_test_app_with_pool() -> (Router, PgPool) {
    let pool = get_test_db_pool().await;

    let rp_id = "localhost";
    let rp_origin = Url::parse("https://localhost").expect("Invalid URL");
    let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
    let webauthn = Arc::new(builder.build().expect("Invalid configuration"));

    let mut rate_limit = back_end::rate_limit::RateLimitState::new();
    rate_limit.disabled = true;

    back_end::exports_db::init_exports_dir()
        .await
        .expect("Failed to initialize exports directory");

    let state = AppState {
        postgres: pool.clone(),
        rate_limit,
        metrics: back_end::metrics::Metrics::new(),
        mongodb: back_end::logs_db::init_mongodb()
            .await
            .expect("Failed to initialize MongoDB"),
        webauthn,
        google_oauth: None,
        oauth_state_store: Arc::new(handlers::OAuthStateStore::default()),
        user_cache: moka::future::Cache::builder()
            .max_capacity(50)
            .time_to_live(std::time::Duration::from_secs(300))
            .build(),
    };

    let app = Router::new()
        .route("/auth/register", post(test_register_handler))
        .route("/auth/login", post(test_login_handler))
        .route("/auth/verify", post(handlers::verify_token))
        .route("/auth/me", get(handlers::get_current_user))
        .route("/auth/invitations/send", post(test_invite_handler))
        .route(
            "/auth/invitations/accept",
            post(test_accept_invitation_handler),
        )
        .route("/companies/{company_id}", get(handlers::get_company))
        .route("/companies/{company_id}", put(handlers::update_company))
        .route(
            "/companies/{company_id}/export",
            post(handlers::export_company_data),
        )
        .route("/companies/{company_id}", delete(handlers::delete_company))
        .route(
            "/companies/{company_id}/confirm-deletion",
            post(handlers::confirm_company_deletion),
        )
        .route(
            "/companies/{company_id}/validate-deletion-token",
            get(handlers::validate_company_deletion_token),
        )
        .route(
            "/companies/{company_id}/logo",
            post(handlers::upload_company_logo),
        )
        .route(
            "/companies/{company_id}/logo",
            get(handlers::get_company_logo),
        )
        .route(
            "/companies/{company_id}/logo",
            delete(handlers::delete_company_logo),
        )
        .with_state(state);

    (app, pool)
}

async fn setup_test_app() -> Router {
    let pool = get_test_db_pool().await;

    // No cleanup here - tests should use unique identifiers to avoid conflicts

    let rp_id = "localhost";
    let rp_origin = Url::parse("https://localhost").expect("Invalid URL");
    let builder = WebauthnBuilder::new(rp_id, &rp_origin).expect("Invalid configuration");
    let webauthn = Arc::new(builder.build().expect("Invalid configuration"));

    let mut rate_limit = back_end::rate_limit::RateLimitState::new();
    rate_limit.disabled = true;

    back_end::exports_db::init_exports_dir()
        .await
        .expect("Failed to initialize exports directory");

    let state = AppState {
        postgres: pool,
        rate_limit,
        metrics: back_end::metrics::Metrics::new(),
        mongodb: back_end::logs_db::init_mongodb()
            .await
            .expect("Failed to initialize MongoDB"),
        webauthn,
        google_oauth: None,
        oauth_state_store: Arc::new(handlers::OAuthStateStore::default()),
        user_cache: moka::future::Cache::builder()
            .max_capacity(50)
            .time_to_live(std::time::Duration::from_secs(300))
            .build(),
    };

    let app = Router::new()
        .route("/auth/register", post(test_register_handler))
        .route("/auth/login", post(test_login_handler))
        .route("/auth/verify", post(handlers::verify_token))
        .route("/auth/me", get(handlers::get_current_user))
        .route("/auth/invitations/send", post(test_invite_handler))
        .route(
            "/auth/invitations/accept",
            post(test_accept_invitation_handler),
        )
        .route("/companies/{company_id}", get(handlers::get_company))
        .route("/companies/{company_id}", put(handlers::update_company))
        .route(
            "/companies/{company_id}/export",
            post(handlers::export_company_data),
        )
        .route("/companies/{company_id}", delete(handlers::delete_company))
        .route(
            "/companies/{company_id}/validate-deletion-token",
            get(handlers::validate_company_deletion_token),
        )
        .route(
            "/companies/{company_id}/confirm-deletion",
            post(handlers::confirm_company_deletion),
        )
        .route(
            "/companies/{company_id}/logo",
            post(handlers::upload_company_logo),
        )
        .route(
            "/companies/{company_id}/logo",
            get(handlers::get_company_logo),
        )
        .route(
            "/companies/{company_id}/logo",
            delete(handlers::delete_company_logo),
        )
        .with_state(state);

    app
}

async fn make_request(
    app: &mut Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let body_bytes = if let Some(b) = body {
        serde_json::to_vec(&b).unwrap()
    } else {
        Vec::new()
    };

    let mut request = match method {
        "GET" => Request::builder().method("GET"),
        "POST" => Request::builder().method("POST"),
        "PUT" => Request::builder().method("PUT"),
        "DELETE" => Request::builder().method("DELETE"),
        _ => panic!("Unsupported method: {}", method),
    };

    request = request.uri(path).header("content-type", "application/json");

    if let Some(t) = token {
        request = request.header("authorization", format!("Bearer {}", t));
    }

    let request = request
        .body(Body::from(body_bytes))
        .expect("Failed to build request");

    // Clone the app before using oneshot to avoid consuming it
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("Failed to send request");
    let status = response.status();

    let body_bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let json_body: Value = if body_bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or(json!({}))
    };

    (status, json_body)
}

#[tokio::test]
async fn test_register_user_success() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let (status, body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("adminuser1{}@example.com", unique_id),
            "first_name": "Admin",
            "last_name": "User",
            "password": "SecurePassword123!",
            "company_name": "Test Company",
            "company_address": "123 Main St"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["token"].is_string());
    assert_eq!(
        body["user"]["email"],
        format!("adminuser1{}@example.com", unique_id)
    );
    assert_eq!(body["user"]["role"], "company_manager");
}

#[tokio::test]
async fn test_register_user_short_password() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let (status, body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("admin{}@example.com", unique_id),
            "first_name": "Admin",
            "last_name": "User",
            "password": "short",
            "company_name": "Test Company",
            "company_address": "123 Main St"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn test_register_user_missing_fields() {
    let mut app = setup_test_app().await;

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": "",
            "first_name": "",
            "last_name": "",
            "password": "",
            "company_name": "",
            "company_address": ""
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_login_user_success() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let register_response = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("user_login_success{}@example.com", unique_id),
            "first_name": "Test",
            "last_name": "User",
            "password": "TestPassword123!",
            "company_name": "Test Co",
            "company_address": "456 Oak Ave"
        })),
        None,
    )
    .await;

    assert_eq!(register_response.0, StatusCode::CREATED);

    let (login_status, login_body) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": format!("user_login_success{}@example.com", unique_id),
            "password": "TestPassword123!"
        })),
        None,
    )
    .await;

    assert_eq!(login_status, StatusCode::OK);
    assert!(login_body["token"].is_string());
    assert_eq!(
        login_body["user"]["email"],
        format!("user_login_success{}@example.com", unique_id)
    );
}

#[tokio::test]
async fn test_login_user_invalid_password() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let _register_response = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("user{}@example.com", unique_id),
            "first_name": "Test",
            "last_name": "User",
            "password": "CorrectPassword123!",
            "company_name": "Test Co",
            "company_address": "456 Oak Ave"
        })),
        None,
    )
    .await;

    let (login_status, _login_body) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": format!("user{}@example.com", unique_id),
            "password": "WrongPassword1!"
        })),
        None,
    )
    .await;

    assert_eq!(login_status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_user_nonexistent() {
    let mut app = setup_test_app().await;

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": "nonexistent@example.com",
            "password": "SomePassword123!"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_current_user_without_token() {
    let mut app = setup_test_app().await;

    let (status, _body) = make_request(&mut app, "GET", "/auth/me", None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_current_user_with_valid_token() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let register_response = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("user6{}@example.com", unique_id),
            "first_name": "Test",
            "last_name": "User",
            "password": "TestPassword123!",
            "company_name": "Test Co",
            "company_address": "456 Oak Ave"
        })),
        None,
    )
    .await;

    let token = register_response.1["token"].as_str().unwrap();
    let (status, body) = make_request(&mut app, "GET", "/auth/me", None, Some(token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], format!("user6{}@example.com", unique_id));
}

#[tokio::test]
async fn test_get_current_user_with_invalid_token() {
    let mut app = setup_test_app().await;

    let (status, _body) =
        make_request(&mut app, "GET", "/auth/me", None, Some("invalid_token")).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invite_user_by_admin() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let register_response = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("admin5{}@example.com", unique_id),
            "first_name": "Admin",
            "last_name": "User",
            "password": "AdminPassword123!",
            "company_name": "Test Co2",
            "company_address": "123 Main St"
        })),
        None,
    )
    .await;

    if let Some(token) = register_response.1["token"].as_str() {
        let (status, body) = make_request(
            &mut app,
            "POST",
            "/auth/invitations/send",
            Some(json!({
                "email": format!("newuser2{}@example.com", unique_id)
            })),
            Some(token),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["email"], format!("newuser2{}@example.com", unique_id));
        assert!(body["expires_at"].is_string());
    } else {
        eprintln!("Registration response body: {}", register_response.1);
        panic!("Failed to get token from registration response");
    }
}

#[tokio::test]
async fn test_invite_user_missing_email() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let register_response = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("admin_invite_user_missing_email{}@example.com", unique_id),
            "first_name": "Admin",
            "last_name": "User",
            "password": "AdminPassword123!",
            "company_name": "Test Co",
            "company_address": "123 Main St"
        })),
        None,
    )
    .await;

    let admin_token = register_response.1["token"].as_str().unwrap();

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/auth/invitations/send",
        Some(json!({
            "email": ""
        })),
        Some(admin_token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_invite_user_by_non_admin() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let admin_response = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("adminuser2{}@example.com", unique_id),
            "first_name": "Admin",
            "last_name": "User",
            "password": "AdminPassword123!",
            "company_name": "Test Co",
            "company_address": "123 Main St"
        })),
        None,
    )
    .await;

    assert_eq!(admin_response.0, StatusCode::CREATED);
    let admin_token = admin_response.1["token"].as_str().unwrap();

    let invite_response = make_request(
        &mut app,
        "POST",
        "/auth/invitations/send",
        Some(json!({
            "email": format!("memberuser1{}@example.com", unique_id)
        })),
        Some(admin_token),
    )
    .await;

    assert_eq!(invite_response.0, StatusCode::CREATED);
    let invitation_id = invite_response.1["id"].as_str().unwrap().to_string();

    let pool = get_test_db_pool().await;

    let invitation = db::get_invitation_by_token(&pool, &invitation_id)
        .await
        .ok()
        .flatten();

    let actual_token = if let Some(inv) = invitation {
        inv.token
    } else {
        let all_invites =
            sqlx::query_as::<_, (String,)>("SELECT token FROM invitations WHERE email = $1")
                .bind(format!("memberuser1{}@example.com", unique_id))
                .fetch_one(&pool)
                .await
                .map(|row| row.0)
                .unwrap_or_else(|_| invitation_id.clone());
        all_invites
    };

    let accept_response = make_request(
        &mut app,
        "POST",
        "/auth/invitations/accept",
        Some(json!({
            "token": actual_token,
            "first_name": "Member",
            "last_name": "User",
            "password": "MemberPassword123!"
        })),
        None,
    )
    .await;

    assert_eq!(accept_response.0, StatusCode::CREATED);
    let member_token = accept_response.1["token"].as_str().unwrap();

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/auth/invitations/send",
        Some(json!({
            "email": format!("anotheruser{}@example.com", unique_id)
        })),
        Some(member_token),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_accept_invitation_missing_fields() {
    let mut app = setup_test_app().await;

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/auth/invitations/accept",
        Some(json!({
            "token": "",
            "first_name": "",
            "last_name": "",
            "password": ""
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_accept_invitation_short_password() {
    let mut app = setup_test_app().await;

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/auth/invitations/accept",
        Some(json!({
            "token": "token123",
            "first_name": "Jane",
            "last_name": "Doe",
            "password": "short"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_accept_invitation_invalid_token() {
    let mut app = setup_test_app().await;

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/auth/invitations/accept",
        Some(json!({
            "token": "invalid_token",
            "first_name": "Jane",
            "last_name": "Doe",
            "password": "SecurePass123!"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_complete_registration_and_login_flow() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let register_response = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("admin3{}@example.com", unique_id),
            "first_name": "Admin",
            "last_name": "User",
            "password": "AdminPassword123!",
            "company_name": "Tech Corp",
            "company_address": "789 Elm St"
        })),
        None,
    )
    .await;

    assert_eq!(register_response.0, StatusCode::CREATED);
    let admin_token = register_response.1["token"].as_str().unwrap().to_string();

    let (login_status, login_body) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": format!("admin3{}@example.com", unique_id),
            "password": "AdminPassword123!"
        })),
        None,
    )
    .await;

    assert_eq!(login_status, StatusCode::OK);
    assert_eq!(login_body["user"]["role"], "company_manager");

    let (me_status, me_body) =
        make_request(&mut app, "GET", "/auth/me", None, Some(&admin_token)).await;

    assert_eq!(me_status, StatusCode::OK);
    assert_eq!(me_body["email"], format!("admin3{}@example.com", unique_id));
    assert_eq!(me_body["role"], "company_manager");
}

#[tokio::test]
async fn test_register_duplicate_email() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let _first_register = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("user{}@example.com", unique_id),
            "first_name": "User",
            "last_name": "One",
            "password": "Password123!",
            "company_name": "Company One",
            "company_address": "123 Main St"
        })),
        None,
    )
    .await;

    let (status, body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("user{}@example.com", unique_id),
            "first_name": "User",
            "last_name": "Two",
            "password": "Password456!",
            "company_name": "Company Two",
            "company_address": "456 Oak Ave"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn test_security_logging_on_successful_registration() {
    let mut app = setup_test_app().await;
    let pool = get_test_db_pool().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let (status, _) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("newuser{}@example.com", unique_id),
            "first_name": "New",
            "last_name": "User",
            "password": "SecurePass123!",
            "company_name": "Test Company",
            "company_address": "123 Test St"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    let user = db::get_user_by_email(&pool, &format!("newuser{}@example.com", unique_id))
        .await
        .unwrap()
        .unwrap();
    let user_id = &user.id;
    let logs = db::get_security_logs_by_user(&pool, user_id, 10)
        .await
        .unwrap();

    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].event_type, "registration");
    assert_eq!(logs[0].user_id, Some(user_id.to_string()));
    assert_eq!(
        logs[0].email,
        Some(format!("newuser{}@example.com", unique_id))
    );
    assert!(logs[0].success);
}

#[tokio::test]
async fn test_security_logging_on_successful_login() {
    let mut app = setup_test_app().await;
    let pool = get_test_db_pool().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let _ = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("logintest{}@example.com", unique_id),
            "first_name": "Login",
            "last_name": "Test",
            "password": "MyPassword123!",
            "company_name": "Login Company",
            "company_address": "456 Login Ave"
        })),
        None,
    )
    .await;

    let (status, _) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": format!("logintest{}@example.com", unique_id),
            "password": "MyPassword123!"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let user = db::get_user_by_email(&pool, &format!("logintest{}@example.com", unique_id))
        .await
        .unwrap()
        .unwrap();
    let user_id = &user.id;
    let logs = db::get_security_logs_by_user(&pool, user_id, 10)
        .await
        .unwrap();

    assert!(logs.len() >= 2);
    assert_eq!(logs[0].event_type, "login_success");
    assert_eq!(logs[0].user_id, Some(user_id.to_string()));
    assert!(logs[0].success);
}

#[tokio::test]
async fn test_security_logging_on_failed_login_wrong_password() {
    let mut app = setup_test_app().await;
    let pool = get_test_db_pool().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let _ = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("failtest{}@example.com", unique_id),
            "first_name": "Fail",
            "last_name": "Test",
            "password": "CorrectPass123!",
            "company_name": "Fail Company",
            "company_address": "789 Fail Rd"
        })),
        None,
    )
    .await;

    let (status, body) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": format!("failtest{}@example.com", unique_id),
            "password": "WrongPassword123!"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Invalid email or password")
    );

    let user = db::get_user_by_email(&pool, &format!("failtest{}@example.com", unique_id))
        .await
        .unwrap()
        .unwrap();
    let user_id = &user.id;
    let logs = db::get_security_logs_by_user(&pool, user_id, 10)
        .await
        .unwrap();

    let failed_login = logs.iter().find(|l| l.event_type == "login_failed");
    assert!(failed_login.is_some());

    let failed_log = failed_login.unwrap();
    assert_eq!(failed_log.user_id, Some(user_id.to_string()));
    assert_eq!(
        failed_log.email,
        Some(format!("failtest{}@example.com", unique_id))
    );
    assert!(!failed_log.success);
    assert_eq!(failed_log.details, Some("Invalid password".to_string()));
}

#[tokio::test]
async fn test_security_logging_on_failed_login_user_not_found() {
    let mut app = setup_test_app().await;
    let pool = get_test_db_pool().await;

    let (status, body) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": "nonexistent@example.com",
            "password": "SomePassword123!"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Invalid email or password")
    );

    sleep(std::time::Duration::from_secs(2)).await;

    let logs = db::get_recent_security_logs(&pool, Some("login_failed".to_string()), 10)
        .await
        .unwrap();

    let failed_login = logs
        .iter()
        .find(|l| l.email == Some("nonexistent@example.com".to_string()));
    assert!(failed_login.is_some());

    let failed_log = failed_login.unwrap();
    assert_eq!(failed_log.user_id, None);
    assert!(!failed_log.success);
    assert_eq!(failed_log.details, Some("User not found".to_string()));
}

#[tokio::test]
async fn test_security_logging_on_invitation_sent() {
    let mut app = setup_test_app().await;
    let pool = get_test_db_pool().await;

    let admin_email = format!(
        "admin{}@company.com",
        Uuid::new_v4().to_string()[..8].to_string()
    );
    let employee_email = format!(
        "employee{}@company.com",
        Uuid::new_v4().to_string()[..8].to_string()
    );

    let (_, register_response) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": admin_email,
            "first_name": "Admin",
            "last_name": "User",
            "password": "AdminPass123!",
            "company_name": "Invite Company",
            "company_address": "321 Invite St"
        })),
        None,
    )
    .await;

    let admin_token = register_response["token"].as_str().unwrap();

    let (status, _) = make_request(
        &mut app,
        "POST",
        "/auth/invitations/send",
        Some(json!({
            "email": employee_email
        })),
        Some(admin_token),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    let admin = db::get_user_by_email(&pool, &admin_email)
        .await
        .unwrap()
        .unwrap();
    let admin_id = &admin.id;
    let logs = db::get_security_logs_by_user(&pool, admin_id, 10)
        .await
        .unwrap();

    let invite_log = logs.iter().find(|l| l.event_type == "invitation_sent");
    assert!(invite_log.is_some());

    let log = invite_log.unwrap();
    assert_eq!(log.user_id, Some(admin_id.to_string()));
    assert_eq!(log.email, Some(employee_email));
    assert!(log.success);
}

#[tokio::test]
async fn test_security_logging_on_invitation_accepted() {
    let mut app = setup_test_app().await;
    let pool = get_test_db_pool().await;

    let (_, register_response) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("boss{}@company.com", Uuid::new_v4().to_string()[..8].to_string()),
            "first_name": "Boss",
            "last_name": "User",
            "password": "BossPass123!",
            "company_name": "Accept Company",
            "company_address": "654 Accept Blvd"
        })),
        None,
    )
    .await;

    let admin_token = register_response["token"].as_str().unwrap();
    let employee_email = format!(
        "employee{}@company.com",
        Uuid::new_v4().to_string()[..8].to_string()
    );
    make_request(
        &mut app,
        "POST",
        "/auth/invitations/send",
        Some(json!({
            "email": employee_email
        })),
        Some(admin_token),
    )
    .await;

    let invitation_token: String =
        sqlx::query_scalar("SELECT token FROM invitations WHERE email = $1")
            .bind(&employee_email)
            .fetch_one(&pool)
            .await
            .unwrap();

    let (status, _) = make_request(
        &mut app,
        "POST",
        "/auth/invitations/accept",
        Some(json!({
            "token": invitation_token,
            "first_name": "New",
            "last_name": "Employee",
            "password": "EmployeePass123!"
        })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);

    let employee = db::get_user_by_email(&pool, &employee_email)
        .await
        .unwrap()
        .unwrap();
    let employee_id = &employee.id;
    let logs = db::get_security_logs_by_user(&pool, employee_id, 10)
        .await
        .unwrap();

    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].event_type, "invitation_accepted");
    assert_eq!(logs[0].user_id, Some(employee_id.to_string()));
    assert_eq!(logs[0].email, Some(employee_email));
    assert!(logs[0].success);
}

#[tokio::test]
async fn test_security_logs_order_by_time() {
    let mut app = setup_test_app().await;
    let pool = get_test_db_pool().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let _ = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("timetest{}@example.com", unique_id),
            "first_name": "Time",
            "last_name": "Test",
            "password": "TimePass123!",
            "company_name": "Time Company",
            "company_address": "999 Time Ave"
        })),
        None,
    )
    .await;

    make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": format!("timetest{}@example.com", unique_id),
            "password": "TimePass123!"
        })),
        None,
    )
    .await;

    make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": format!("timetest{}@example.com", unique_id),
            "password": "WrongPass123!"
        })),
        None,
    )
    .await;

    let user = db::get_user_by_email(&pool, &format!("timetest{}@example.com", unique_id))
        .await
        .unwrap()
        .unwrap();
    let user_id = &user.id;
    let logs = db::get_security_logs_by_user(&pool, user_id, 10)
        .await
        .unwrap();

    assert!(logs.len() >= 3);

    for i in 0..logs.len() - 1 {
        assert!(logs[i].created_at >= logs[i + 1].created_at);
    }
}

#[tokio::test]
async fn test_get_company_unauthorized() {
    let mut app = setup_test_app().await;

    let (status, _body) =
        make_request(&mut app, "GET", "/companies/test-company-id", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_update_company_as_company_manager() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("companymanager{}@example.com", unique_id),
            "first_name": "Company",
            "last_name": "Manager",
            "password": "SecurePass123!",
            "company_name": "Test Company ABC",
            "company_address": "123 Test St"
        })),
        None,
    )
    .await;

    assert_eq!(register_body["user"]["role"], "company_manager");
    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (status, _body) = make_request(
        &mut app,
        "PUT",
        &format!("/companies/{}", company_id),
        Some(json!({
            "name": "Updated Company Name",
            "address": "456 New Address"
        })),
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_update_company_unauthorized() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("owner1{}@example.com", unique_id),
            "first_name": "Owner",
            "last_name": "One",
            "password": "SecurePass123!",
            "company_name": "Owner Company",
            "company_address": "111 Main St"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();

    let (_, register_body2) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("owner2{}@example.com", unique_id),
            "first_name": "Owner",
            "last_name": "Two",
            "password": "SecurePass123!",
            "company_name": "Other Company",
            "company_address": "222 Other St"
        })),
        None,
    )
    .await;

    let other_company_id = register_body2["user"]["company_id"].as_str().unwrap();

    let (status, _body) = make_request(
        &mut app,
        "PUT",
        &format!("/companies/{}", other_company_id),
        Some(json!({
            "name": "Hacked Company",
            "address": "Hacked Address"
        })),
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_export_company_data() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("exporter_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Export",
            "last_name": "User",
            "password": "SecurePass123!",
            "company_name": format!("Export Company {}", unique_id),
            "company_address": "999 Export Ave"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (status, _body) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_delete_company_without_export_fails() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("deleter_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Delete",
            "last_name": "User",
            "password": "SecurePass123!",
            "company_name": format!("Delete Company {}", unique_id),
            "company_address": "888 Delete Blvd"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (status, body) = make_request(
        &mut app,
        "DELETE",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("export"));
}

#[tokio::test]
async fn test_get_company_details() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("viewer_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "View",
            "last_name": "User",
            "password": "SecurePass123!",
            "company_name": "View Company",
            "company_address": "777 View Rd"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (status, body) = make_request(
        &mut app,
        "GET",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "View Company");
    assert_eq!(body["address"], "777 View Rd");
}

#[tokio::test]
async fn test_export_company_data_returns_structure() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("exportstruct_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Export",
            "last_name": "Struct",
            "password": "SecurePass123!",
            "company_name": format!("Export Struct Company {}", unique_id),
            "company_address": "123 Export St"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (status, body) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["message"].as_str().is_some());
    assert!(body["exported_at"].as_str().is_some());
}

#[tokio::test]
async fn test_delete_company_after_export_succeeds() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("deletetest_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Delete",
            "last_name": "Test",
            "password": "SecurePass123!",
            "company_name": format!("Delete Test Company {}", unique_id),
            "company_address": "456 Delete Ave"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (export_status, body) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;
    println!("Export body: {:?}", body);
    assert_eq!(export_status, StatusCode::OK);

    let (status, body) = make_request(
        &mut app,
        "DELETE",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["message"].as_str().unwrap().contains("Deletion"));
}

#[tokio::test]
async fn test_confirm_company_deletion_invalid_token() {
    let mut app = setup_test_app().await;

    let (status, _body) = make_request(
        &mut app,
        "POST",
        "/companies/test-company-id/confirm-deletion",
        Some(json!({ "token": "invalid-token" })),
        None,
    )
    .await;

    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_login_blocked_after_company_deletion() {
    let (mut app, pool) = setup_test_app_with_pool().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("deletionlogin_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Test",
            "last_name": "User",
            "password": "TestPassword123!",
            "company_name": format!("Deletion Test Co {}", unique_id),
            "company_address": "456 Test Ave"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (export_status, _) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(export_status, StatusCode::OK);

    let (delete_request_status, _) = make_request(
        &mut app,
        "DELETE",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(delete_request_status, StatusCode::OK);

    let row: (String,) = sqlx::query_as("SELECT deletion_token FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let deletion_token = row.0;

    let (confirm_status, _) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/confirm-deletion", company_id),
        Some(json!({ "token": deletion_token })),
        None,
    )
    .await;
    assert_eq!(confirm_status, StatusCode::OK);

    let (login_status, login_body) = make_request(
        &mut app,
        "POST",
        "/auth/login",
        Some(json!({
            "email": email,
            "password": "TestPassword123!"
        })),
        None,
    )
    .await;

    assert_eq!(login_status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        login_body["error"].as_str().unwrap(),
        "Invalid email or password"
    );
}

#[tokio::test]
async fn test_api_calls_blocked_after_company_deletion() {
    let (mut app, pool) = setup_test_app_with_pool().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("deletionapi_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Test",
            "last_name": "User",
            "password": "TestPassword123!",
            "company_name": format!("Deletion API Co {}", unique_id),
            "company_address": "456 API Ave"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (export_status, _) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(export_status, StatusCode::OK);

    let (delete_request_status, _) = make_request(
        &mut app,
        "DELETE",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(delete_request_status, StatusCode::OK);

    let row: (String,) = sqlx::query_as("SELECT deletion_token FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let deletion_token = row.0;

    let (confirm_status, _) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/confirm-deletion", company_id),
        Some(json!({ "token": deletion_token })),
        None,
    )
    .await;
    assert_eq!(confirm_status, StatusCode::OK);

    let (me_status, me_body) = make_request(&mut app, "GET", "/auth/me", None, Some(token)).await;

    assert_eq!(me_status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        me_body["error"].as_str().unwrap(),
        "Invalid or expired token"
    );
}

#[tokio::test]
async fn test_update_company_details() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("updatetest{}@example.com", unique_id),
            "first_name": "Update",
            "last_name": "Test",
            "password": "SecurePass123!",
            "company_name": "Original Name",
            "company_address": "111 Original St"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    let (status, body) = make_request(
        &mut app,
        "PUT",
        &format!("/companies/{}", company_id),
        Some(json!({
            "name": "Updated Company Name",
            "address": "222 Updated Ave"
        })),
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Updated Company Name");
    assert_eq!(body["address"], "222 Updated Ave");
}

#[tokio::test]
async fn test_cannot_update_other_company() {
    let mut app = setup_test_app().await;
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();

    let (_, register_body1) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("company1{}@example.com", unique_id),
            "first_name": "Company",
            "last_name": "One",
            "password": "SecurePass123!",
            "company_name": "Company One",
            "company_address": "111 First St"
        })),
        None,
    )
    .await;

    let token1 = register_body1["token"].as_str().unwrap();

    let (_, register_body2) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": format!("company2{}@example.com", unique_id),
            "first_name": "Company",
            "last_name": "Two",
            "password": "SecurePass123!",
            "company_name": "Company Two",
            "company_address": "222 Second St"
        })),
        None,
    )
    .await;

    let company2_id = register_body2["user"]["company_id"].as_str().unwrap();

    let (status, _) = make_request(
        &mut app,
        "PUT",
        &format!("/companies/{}", company2_id),
        Some(json!({
            "name": "Hacked Name",
            "address": "Hacked Address"
        })),
        Some(token1),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_company_with_expired_export_fails() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("expiretest_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Expire",
            "last_name": "Test",
            "password": "SecurePass123!",
            "company_name": format!("Expire Test Company {}", unique_id),
            "company_address": "123 Expire Ave"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    // Export data
    let (export_status, _) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(export_status, StatusCode::OK);

    // Manually set data_exported_at to 7 hours ago
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .expect("Failed to connect to database");

    sqlx::query(
        r#"
        UPDATE companies
        SET data_exported_at = NOW() - INTERVAL '7 hours'
        WHERE id = $1
        "#,
    )
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("Failed to update test data");

    // Try to request deletion with expired export
    let (status, body) = make_request(
        &mut app,
        "DELETE",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("expired"));
}

#[tokio::test]
async fn test_deletion_confirmation_token_expires_after_6_hours() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("tokenexpire_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Token",
            "last_name": "Expire",
            "password": "SecurePass123!",
            "company_name": format!("Token Expire Company {}", unique_id),
            "company_address": "456 Token Ave"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    // Export data
    let (export_status, _) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(export_status, StatusCode::OK);

    // Request deletion
    let (del_status, _del_body) = make_request(
        &mut app,
        "DELETE",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(del_status, StatusCode::OK);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .expect("Failed to connect to database");

    // Get the deletion token
    let company: (Option<String>,) =
        sqlx::query_as("SELECT deletion_token FROM companies WHERE id = $1")
            .bind(company_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch company");

    let deletion_token = company.0.expect("No deletion token found");

    // Manually set deletion_requested_at to 7 hours ago
    sqlx::query(
        r#"
        UPDATE companies
        SET deletion_requested_at = NOW() - INTERVAL '7 hours'
        WHERE id = $1
        "#,
    )
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("Failed to update test data");

    // Try to confirm deletion with expired token
    let (confirm_status, confirm_body) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/confirm-deletion", company_id),
        Some(json!({ "token": deletion_token })),
        None,
    )
    .await;

    assert_eq!(confirm_status, StatusCode::BAD_REQUEST);
    assert!(confirm_body["error"].as_str().unwrap().contains("expired"));
}

#[tokio::test]
async fn test_validate_deletion_token_expires_after_6_hours() {
    let mut app = setup_test_app().await;

    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let email = format!("validatetoken_{}@example.com", unique_id);

    let (_, register_body) = make_request(
        &mut app,
        "POST",
        "/auth/register",
        Some(json!({
            "email": email,
            "first_name": "Validate",
            "last_name": "Token",
            "password": "SecurePass123!",
            "company_name": format!("Validate Token Company {}", unique_id),
            "company_address": "789 Validate Ave"
        })),
        None,
    )
    .await;

    let token = register_body["token"].as_str().unwrap();
    let company_id = register_body["user"]["company_id"].as_str().unwrap();

    // Export data
    let (export_status, body) = make_request(
        &mut app,
        "POST",
        &format!("/companies/{}/export", company_id),
        None,
        Some(token),
    )
    .await;
    println!("{}", body);
    assert_eq!(export_status, StatusCode::OK);

    // Request deletion
    let (del_status, _) = make_request(
        &mut app,
        "DELETE",
        &format!("/companies/{}", company_id),
        None,
        Some(token),
    )
    .await;
    assert_eq!(del_status, StatusCode::OK);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .expect("Failed to connect to database");

    // Get the deletion token
    let company: (Option<String>,) =
        sqlx::query_as("SELECT deletion_token FROM companies WHERE id = $1")
            .bind(company_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch company");

    let deletion_token = company.0.expect("No deletion token found");

    // Manually set deletion_requested_at to 7 hours ago
    sqlx::query(
        r#"
        UPDATE companies
        SET deletion_requested_at = NOW() - INTERVAL '7 hours'
        WHERE id = $1
        "#,
    )
    .bind(company_id)
    .execute(&pool)
    .await
    .expect("Failed to update test data");

    // Try to validate token with expired request
    let (validate_status, validate_body) = make_request(
        &mut app,
        "GET",
        &format!(
            "/companies/{}/validate-deletion-token?token={}",
            company_id, deletion_token
        ),
        None,
        None,
    )
    .await;

    assert_eq!(validate_status, StatusCode::BAD_REQUEST);
    assert!(validate_body["error"].as_str().unwrap().contains("expired"));
}
