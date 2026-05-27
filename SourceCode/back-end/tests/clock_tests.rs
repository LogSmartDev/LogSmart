use back_end::{
    auth::hash_password,
    db::{self, UserRole},
};
use sqlx::PgPool;

async fn get_test_pool() -> PgPool {
    let connection_string = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://admin:adminpassword@localhost:5432/logsmartdb".to_string());

    PgPool::connect(&connection_string)
        .await
        .expect("Failed to create test db connection")
}

async fn setup_test_pool() -> PgPool {
    get_test_pool().await
}

fn generate_unique_id() -> String {
    uuid::Uuid::new_v4().to_string().replace("-", "")
}

#[tokio::test]
async fn test_clock_in_creates_event() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Clock Test Company {}", test_id),
        "123 Test St".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("clockuser{}@example.com", test_id),
        "Clock".to_string(),
        "User".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    let event = db::clock_in(&pool, &user.id, &company.id)
        .await
        .expect("Failed to clock in");

    assert_eq!(event.user_id, user.id);
    assert_eq!(event.company_id, company.id);
    assert!(event.is_clocked_in());
    assert!(event.clock_out.is_none());
}

#[tokio::test]
async fn test_clock_out_updates_event() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Clock Out Company {}", test_id),
        "456 Test Ave".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("clockout{}@example.com", test_id),
        "Clock".to_string(),
        "Out".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    db::clock_in(&pool, &user.id, &company.id)
        .await
        .expect("Failed to clock in");

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let event = db::clock_out(&pool, &user.id)
        .await
        .expect("Failed to clock out")
        .expect("No clock out event returned");

    assert_eq!(event.user_id, user.id);
    assert!(!event.is_clocked_in());
    assert!(event.clock_out.is_some());
}

#[tokio::test]
async fn test_clock_out_returns_none_when_not_clocked_in() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("No Clock Company {}", test_id),
        "789 Test Blvd".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("noclock{}@example.com", test_id),
        "No".to_string(),
        "Clock".to_string(),
        Some(password_hash),
        Some(company.id),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    let result = db::clock_out(&pool, &user.id)
        .await
        .expect("Database query should succeed");

    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_clock_status_returns_latest() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Status Company {}", test_id),
        "111 Status St".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("status{}@example.com", test_id),
        "Status".to_string(),
        "User".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    db::clock_in(&pool, &user.id, &company.id)
        .await
        .expect("Failed to clock in first time");

    db::clock_out(&pool, &user.id)
        .await
        .expect("Failed to clock out first time");

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let second_event = db::clock_in(&pool, &user.id, &company.id)
        .await
        .expect("Failed to clock in second time");

    let status = db::get_clock_status(&pool, &user.id)
        .await
        .expect("Failed to get clock status")
        .expect("No status returned");

    assert_eq!(status.id, second_event.id);
    assert!(status.is_clocked_in());
}

#[tokio::test]
async fn test_get_recent_clock_events_limit() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Recent Company {}", test_id),
        "222 Recent Rd".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("recent{}@example.com", test_id),
        "Recent".to_string(),
        "User".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    for _ in 0..3 {
        db::clock_in(&pool, &user.id, &company.id)
            .await
            .expect("Failed to clock in");
        db::clock_out(&pool, &user.id)
            .await
            .expect("Failed to clock out");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let events = db::get_recent_clock_events(&pool, &user.id, 2)
        .await
        .expect("Failed to get recent events");

    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn test_get_company_clock_events_basic() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Company Events Co {}", test_id),
        "333 Events Ave".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("companyevents{}@example.com", test_id),
        "Company".to_string(),
        "Events".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    db::clock_in(&pool, &user.id, &company.id)
        .await
        .expect("Failed to clock in");

    db::clock_out(&pool, &user.id)
        .await
        .expect("Failed to clock out");

    let events = db::get_company_clock_events(&pool, &company.id, None, None, None, None, None)
        .await
        .expect("Failed to get company clock events");

    let user_event = events.0.iter().find(|e| e.user_id == user.id);
    assert!(user_event.is_some());

    let event = user_event.unwrap();
    assert_eq!(event.first_name, "Company");
    assert_eq!(event.last_name, "Events");
    assert_eq!(event.email, format!("companyevents{}@example.com", test_id));
}

#[tokio::test]
async fn test_get_company_clock_events_branch_filter() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Branch Filter Co {}", test_id),
        "444 Branch St".to_string(),
    )
    .await
    .expect("Failed to create company");

    let branch_a = db::create_branch(
        &pool,
        company.id.clone(),
        "Branch A".to_string(),
        "A St".to_string(),
    )
    .await
    .expect("Failed to create branch A");

    let branch_b = db::create_branch(
        &pool,
        company.id.clone(),
        "Branch B".to_string(),
        "B St".to_string(),
    )
    .await
    .expect("Failed to create branch B");

    let password_hash = hash_password("TestPassword123").unwrap();

    let user_a = db::create_user(
        &pool,
        format!("usera{}@example.com", test_id),
        "User".to_string(),
        "A".to_string(),
        Some(password_hash.clone()),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user A");
    db::update_user_branch(&pool, &user_a.id, Some(branch_a.id.clone()))
        .await
        .expect("Failed to update user A branch");

    let user_b = db::create_user(
        &pool,
        format!("userb{}@example.com", test_id),
        "User".to_string(),
        "B".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user B");
    db::update_user_branch(&pool, &user_b.id, Some(branch_b.id.clone()))
        .await
        .expect("Failed to update user B branch");

    db::clock_in(&pool, &user_a.id, &company.id)
        .await
        .expect("Failed to clock in user A");
    db::clock_out(&pool, &user_a.id)
        .await
        .expect("Failed to clock out user A");

    db::clock_in(&pool, &user_b.id, &company.id)
        .await
        .expect("Failed to clock in user B");
    db::clock_out(&pool, &user_b.id)
        .await
        .expect("Failed to clock out user B");

    let events_branch_a = db::get_company_clock_events(
        &pool,
        &company.id,
        None,
        None,
        Some(branch_a.id.clone()),
        None,
        None,
    )
    .await
    .expect("Failed to get branch A events");

    assert!(events_branch_a.0.iter().any(|e| e.user_id == user_a.id));
    assert!(!events_branch_a.0.iter().any(|e| e.user_id == user_b.id));

    let events_branch_b = db::get_company_clock_events(
        &pool,
        &company.id,
        None,
        None,
        Some(branch_b.id.clone()),
        None,
        None,
    )
    .await
    .expect("Failed to get branch B events");

    assert!(events_branch_b.0.iter().any(|e| e.user_id == user_b.id));
    assert!(!events_branch_b.0.iter().any(|e| e.user_id == user_a.id));
}

#[tokio::test]
async fn test_get_company_clock_events_date_filter() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Date Filter Co {}", test_id),
        "555 Date Ave".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("datefilter{}@example.com", test_id),
        "Date".to_string(),
        "Filter".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    db::clock_in(&pool, &user.id, &company.id)
        .await
        .expect("Failed to clock in");
    db::clock_out(&pool, &user.id)
        .await
        .expect("Failed to clock out");

    let now = chrono::Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    let one_hour_ahead = now + chrono::Duration::hours(1);

    let events = db::get_company_clock_events(
        &pool,
        &company.id,
        Some(one_hour_ago),
        Some(one_hour_ahead),
        None,
        None,
        None,
    )
    .await
    .expect("Failed to get events with date filter");

    assert!(events.0.iter().any(|e| e.user_id == user.id));

    let far_future = now + chrono::Duration::days(365);
    let empty_events = db::get_company_clock_events(
        &pool,
        &company.id,
        Some(far_future),
        Some(far_future + chrono::Duration::hours(1)),
        None,
        None,
        None,
    )
    .await
    .expect("Failed to get events with future date filter");

    assert!(!empty_events.0.iter().any(|e| e.user_id == user.id));
}

#[tokio::test]
async fn test_clock_service_clock_in_conflict_when_already_in() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Conflict Co {}", test_id),
        "666 Conflict Rd".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("conflict{}@example.com", test_id),
        "Conflict".to_string(),
        "User".to_string(),
        Some(password_hash),
        Some(company.id.clone()),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    let _first = back_end::services::ClockService::clock_in(&pool, &user.id, &company.id)
        .await
        .expect("First clock in should succeed");

    let second = back_end::services::ClockService::clock_in(&pool, &user.id, &company.id).await;

    assert!(second.is_err());
    let err = second.unwrap_err();
    // Check that it's a Conflict error
    assert!(matches!(err, back_end::error::AppError::Conflict(_)));
}

#[tokio::test]
async fn test_clock_service_clock_out_bad_request_when_not_in() {
    let pool = setup_test_pool().await;
    let test_id = generate_unique_id();

    let company = db::create_company(
        &pool,
        format!("Not In Co {}", test_id),
        "777 Not In Blvd".to_string(),
    )
    .await
    .expect("Failed to create company");

    let password_hash = hash_password("TestPassword123").unwrap();
    let user = db::create_user(
        &pool,
        format!("notin{}@example.com", test_id),
        "Not".to_string(),
        "In".to_string(),
        Some(password_hash),
        Some(company.id),
        UserRole::Staff,
    )
    .await
    .expect("Failed to create user");

    let result = back_end::services::ClockService::clock_out(&pool, &user.id).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    // Check that it's a BadRequest error
    assert!(matches!(err, back_end::error::AppError::BadRequest(_)));
}
