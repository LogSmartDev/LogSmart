use anyhow::Context;
use axum::routing::{delete, get, post, put};
use axum::{Router, middleware};
use back_end::exports_db;
use back_end::logs_db;
use back_end::{AppState, api_docs::ApiDoc, db, handlers, rate_limit};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use url::Url;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use tower_http::trace::TraceLayer;
use tower_http::compression::CompressionLayer;

const VARS: [&str; 12] = [
    "JWT_SECRET",
    "SMTP_USERNAME",
    "SMTP_PASSWORD",
    "SMTP_SERVER",
    "SMTP_FROM_EMAIL",
    "GOOGLE_CLIENT_SECRET",
    "GOOGLE_CLIENT_ID",
    "POSTGRES_PASSWORD",
    "POSTGRES_USER",
    "MONGODB_URI",
    "FRONTEND_URL",
    "OPENROUTER_API_KEY",
];

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    for var in &VARS {
        unsafe {
            load_secret(var);
        }
    }

    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "text".to_string());

    if log_format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_target(true)
            .with_current_span(false)
            .with_thread_ids(true)
            .with_line_number(true)
            .with_file(true)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_target(true)
            .with_thread_ids(true)
            .with_line_number(true)
            .with_file(true)
            .init();
    }

    let connect_options = PgConnectOptions::new()
        .host(&std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string()))
        .port(
            std::env::var("POSTGRES_PORT")
                .unwrap_or_else(|_| "5432".to_string())
                .parse()
                .unwrap_or(5432),
        )
        .username(
            &std::env::var("POSTGRES_USER").expect("Required variable: POSTGRES_USER not set"),
        )
        .password(
            &std::env::var("POSTGRES_PASSWORD")
                .expect("Required variable: POSTGRES_PASSWORD not set"),
        )
        .database(&std::env::var("POSTGRES_DB").unwrap_or_else(|_| "logsmartdb".to_string()));

    let auth_db_postgres_pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .max_lifetime(Some(std::time::Duration::from_secs(1800)))
        .idle_timeout(Some(std::time::Duration::from_secs(600)))
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect_with(connect_options)
        .await
        .with_context(|| "Failed to create auth_db_postgres_pool")
        .expect("Cannot create authentication db");

    db::init_db(&auth_db_postgres_pool)
        .await
        .expect("Failed to initialize database");

    exports_db::init_exports_dir()
        .await
        .expect("Failed to initialize exports directory");
    exports_db::spawn_cleanup_task();

    let rate_limit_state = rate_limit::RateLimitState::new();
    rate_limit_state.clone().spawn_cleanup_task();

    let metrics = back_end::metrics::Metrics::new();
    metrics.clone().spawn_logging_task();

    let google_oauth = {
        let client_id = std::env::var("GOOGLE_CLIENT_ID");
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET");
        let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI");
        let issuer_url = std::env::var("GOOGLE_ISSUER_URL");

        if let (Ok(id), Ok(secret), Ok(uri), Ok(issuer)) =
            (client_id, client_secret, redirect_uri, issuer_url)
        {
            match back_end::services::GoogleOAuthClient::new(id, secret, uri.clone(), issuer).await
            {
                Ok(client) => {
                    tracing::info!(
                        "Google OAuth client initialized successfully with redirect_uri: {}",
                        uri
                    );
                    Some(client)
                }
                Err(e) => {
                    tracing::error!("Failed to initialize Google OAuth client: {:?}", e);
                    None
                }
            }
        } else {
            tracing::warn!("Google OAuth not configured - missing environment variables");
            None
        }
    };

    let user_cache = moka::future::Cache::builder()
        .max_capacity(10_000)
        .time_to_live(std::time::Duration::from_secs(900)) // 15 minutes
        .build();

    let state = AppState {
        postgres: auth_db_postgres_pool,
        rate_limit: rate_limit_state.clone(),
        metrics,
        mongodb: logs_db::init_mongodb()
            .await
            .expect("Failed to initialize MongoDB"),
        webauthn: std::sync::Arc::new(
            webauthn_rs::WebauthnBuilder::new(
                &std::env::var("RP_ID").unwrap_or_else(|_| "localhost".to_string()),
                &Url::parse(
                    &std::env::var("RP_ORIGIN")
                        .unwrap_or_else(|_| "http://localhost:5173".to_string()),
                )
                .expect("Invalid RP origin"),
            )
            .expect("Invalid configuration")
            .rp_name("LogSmart")
            .build()
            .expect("Invalid configuration"),
        ),
        google_oauth,
        oauth_state_store: std::sync::Arc::new(handlers::OAuthStateStore::new()),
        user_cache,
    };

    let api_routes = Router::new()
        .route("/health", get(handlers::basic_health_check))
        .route("/auth/register", post(handlers::register_company_admin))
        .route("/auth/login", post(handlers::login))
        .route("/auth/verify", post(handlers::verify_token))
        .route(
            "/auth/google/initiate",
            get(handlers::initiate_google_login),
        )
        .route("/auth/google/callback", get(handlers::google_callback))
        .route("/auth/google/link", post(handlers::link_google_account))
        .route(
            "/auth/google/link/confirm",
            post(handlers::confirm_google_link),
        )
        .route(
            "/auth/google/unlink",
            delete(handlers::unlink_google_account),
        )
        .route(
            "/auth/invitations/accept",
            post(handlers::accept_invitation),
        )
        .route("/auth/invitations/cancel", put(handlers::cancel_invitation))
        .route(
            "/auth/invitations/details",
            get(handlers::get_invitation_details),
        )
        .route(
            "/auth/password/request-reset",
            post(handlers::request_password_reset),
        )
        .route("/auth/password/reset", post(handlers::reset_password))
        .route("/auth/me", get(handlers::get_current_user))
        .route(
            "/auth/passkey/register/start",
            post(handlers::start_passkey_registration),
        )
        .route(
            "/auth/passkey/register/finish",
            post(handlers::finish_passkey_registration),
        )
        .route(
            "/auth/passkey/login/start",
            post(handlers::start_passkey_login),
        )
        .route(
            "/auth/passkey/login/discoverable/start",
            post(handlers::start_discoverable_passkey_login),
        )
        .route(
            "/auth/passkey/login/discoverable/finish",
            post(handlers::finish_discoverable_passkey_login),
        )
        .route(
            "/auth/passkey/login/finish",
            post(handlers::finish_passkey_login),
        )
        .route("/auth/passkeys", get(handlers::list_passkeys))
        .route(
            "/auth/passkeys/{passkey_id}",
            delete(handlers::delete_passkey),
        )
        .route("/auth/profile", put(handlers::update_profile))
        .route("/auth/invitations/send", post(handlers::invite_user))
        .route(
            "/auth/invitations/pending",
            get(handlers::get_pending_invitations),
        )
        .route("/auth/company/members", get(handlers::get_company_members))
        .route("/auth/company/branches", post(handlers::create_branch))
        .route("/auth/company/branches", get(handlers::list_branches))
        .route("/auth/company/branches", put(handlers::update_branch))
        .route(
            "/auth/company/branches/request-deletion",
            post(handlers::request_branch_deletion),
        )
        .route(
            "/auth/company/branches/confirm-deletion",
            post(handlers::confirm_branch_deletion),
        )
        .route(
            "/auth/admin/update-member",
            put(handlers::admin_update_member_profile),
        )
        .route(
            "/auth/admin/remove-member",
            delete(handlers::admin_delete_member),
        )
        .route(
            "/auth/profile-picture",
            post(handlers::upload_profile_picture),
        )
        .route(
            "/auth/profile-picture",
            delete(handlers::delete_profile_picture_handler),
        )
        .route(
            "/auth/profile-picture/{user_id}",
            get(handlers::get_profile_picture),
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
        .route("/companies/{company_id}", get(handlers::get_company))
        .route("/companies/{company_id}", put(handlers::update_company))
        .route(
            "/companies/{company_id}/export",
            post(handlers::export_company_data),
        )
        .route(
            "/companies/{company_id}/export/download/{filename}",
            get(handlers::download_export),
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
        .route("/logs/templates", post(handlers::add_template))
        .route("/logs/templates", get(handlers::get_template))
        .route("/logs/templates/all", get(handlers::get_all_templates))
        .route("/logs/templates/update", put(handlers::update_template))
        .route("/logs/templates/rename", put(handlers::rename_template))
        .route("/logs/templates", delete(handlers::delete_template))
        .route(
            "/logs/templates/versions",
            get(handlers::get_template_versions),
        )
        .route(
            "/logs/templates/versions/restore",
            post(handlers::restore_template_version),
        )
        .route("/logs/entries/due", get(handlers::list_due_forms_today))
        .route("/logs/entries", post(handlers::create_log_entry))
        .route("/logs/entries", get(handlers::list_user_log_entries))
        .route(
            "/logs/admin/entries",
            get(handlers::list_company_log_entries::<std::hash::RandomState>),
        )
        .route("/logs/entries/{entry_id}", get(handlers::get_log_entry))
        .route("/logs/entries/{entry_id}", put(handlers::update_log_entry))
        .route(
            "/logs/entries/{entry_id}",
            delete(handlers::delete_log_entry),
        )
        .route("/reports/runs", post(handlers::create_report_run))
        .route("/reports/runs", get(handlers::list_report_runs))
        .route(
            "/reports/runs/{report_id}/use",
            post(handlers::use_report_run),
        )
        .route(
            "/reports/runs/{report_id}",
            delete(handlers::delete_report_run),
        )
        .route(
            "/logs/entries/{entry_id}/submit",
            post(handlers::submit_log_entry),
        )
        .route(
            "/logs/entries/{entry_id}/unsubmit",
            post(handlers::unsubmit_log_entry),
        )
        .route("/llm/generate-layout", post(handlers::generate_layout))
        .route("/clock/in", post(handlers::clock_in))
        .route("/clock/out", post(handlers::clock_out))
        .route("/clock/status", get(handlers::get_clock_status))
        .route("/clock/company", get(handlers::get_company_clock_events))
        .route("/health/database", get(handlers::get_db_health))
        .route("/health/slow-queries", get(handlers::get_db_slow_queries))
        .route("/health/index-usage", get(handlers::get_db_index_usage))
        .route("/health/table-sizes", get(handlers::get_db_table_sizes))
        .route("/health/rate-limits", get(handlers::get_rate_limit_status))
        .route("/security/logs", get(handlers::get_security_logs))
        .route(
            "/security/logs/export",
            get(handlers::export_security_logs_csv),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_middleware,
        ))
        .with_state(state);

    let swagger_router: Router = SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into();

    // Parse allowed origins from environment variable or use defaults
    let allowed_origins_str = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173,http://logsmart.app,https://logsmart.app".to_string());
    
    let allowed_origins: Vec<_> = allowed_origins_str
        .split(',')
        .filter_map(|origin| {
            let trimmed = origin.trim();
            if !trimmed.is_empty() {
                trimmed.parse().ok()
            } else {
                None
            }
        })
        .collect();

    if allowed_origins.is_empty() {
        tracing::warn!("No valid CORS origins configured, using defaults");
    }

    let app = swagger_router.merge(api_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(allowed_origins)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                ])
                .allow_credentials(true),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6767")
        .await
        .expect("Failed to bind to port 6767");

    tracing::info!("Server running on http://0.0.0.0:6767");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .expect("Server error");
}

unsafe fn load_secret(key: &str) {
    let file_env_key = format!("{key}_FILE");

    if let Ok(path) = std::env::var(&file_env_key) {
        match std::fs::read_to_string(&path) {
            Ok(secret) => {
                let value = secret.trim().to_string();
                unsafe {
                    std::env::set_var(key, value);
                }
                println!("Loaded secret for {key} from file: {path}");
            }
            Err(e) => {
                eprintln!("Warning: Could not read secret file {path}: {e}");
            }
        }
    }
}
