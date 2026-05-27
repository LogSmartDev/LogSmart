//! Common testing utilities for the backend test suite
//!
//! This module provides shared utilities for test setup, teardown,
//! database management, and common test helpers.

pub mod config;
pub mod factories;
pub mod mocks;

use sqlx::PgPool;

pub use factories::*;

/// Sets up a test database with test data.
///
/// # Panics
/// Panics if the database connection fails or if cleanup queries fail.
pub async fn setup_test_db() -> PgPool {
    // Use environment variable for database URL, with fallback to test database
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://admin:adminpassword@localhost:5432/logsmartdb".to_string());

    PgPool::connect(&database_url)
        .await
        .expect("Failed to create test database pool")
}

/// Test helper for running database migrations.
///
/// # Errors
/// Returns an error if migrations fail to run.
pub async fn setup_test_database(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    // Run migrations on test database
    sqlx::migrate!("./migrations").run(pool).await
}

/// Test helper for cleaning up test database.
///
/// # Errors
/// Returns an error if the truncate query fails.
pub async fn cleanup_test_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Clean up all tables in reverse order of dependencies
    sqlx::query("TRUNCATE TABLE security_logs, passkey_sessions, passkeys, invitations, companies, users CASCADE")
        .execute(pool)
        .await?;

    Ok(())
}

/// Test helper to create isolated test database connection.
///
/// # Panics
/// Panics if the database connection fails.
pub async fn create_test_pool() -> PgPool {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://admin:adminpassword@localhost:5432/logsmartdb".to_string());

    sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create test database pool")
}

/// Macro for test setup and teardown
#[macro_export]
macro_rules! test_with_db {
    ($test_name:ident) => {
        #[tokio::test]
        async fn $test_name() {
            let pool = $crate::common::create_test_pool().await;
            $crate::common::setup_test_database(&pool)
                .await
                .expect("Failed to setup test database");

            let result = {
                // Run actual test logic here
                // This will be replaced by specific test implementation
                Ok::<(), Box<dyn std::error::Error>>(())
            };

            $crate::common::cleanup_test_database(&pool)
                .await
                .expect("Failed to cleanup test database");

            result.expect("Test failed");
        }
    };
}
