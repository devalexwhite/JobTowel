use jobtowel::db;
use sqlx::migrate::{Migrate, MigrateError};

async fn database() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite://{}",
        dir.path().join("test.db").display()
    ))
    .await
    .unwrap();
    (pool, dir)
}

#[tokio::test]
async fn fresh_database_contains_only_application_tables() {
    let (pool, _dir) = database().await;
    db::migrate(&pool).await.unwrap();
    db::migrate(&pool).await.unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        tables,
        [
            "_sqlx_migrations",
            "event_people",
            "events",
            "roles",
            "sessions",
            "users"
        ]
    );
}

#[tokio::test]
async fn retired_demo_history_does_not_block_existing_databases() {
    let (pool, dir) = database().await;
    pool.acquire()
        .await
        .unwrap()
        .ensure_migrations_table()
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (20261006000000, 'retired demo', true, X'00', 0)",
    ).execute(&pool).await.unwrap();
    db::migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (username, name, email, password_hash) VALUES ('existing', 'Existing User', 'existing@example.com', ?)")
        .bind(jobtowel::models::auth::DUMMY_PASSWORD_HASH).execute(&pool).await.unwrap();
    pool.close().await;
    let reopened = db::connect(&format!(
        "sqlite://{}",
        dir.path().join("test.db").display()
    ))
    .await
    .unwrap();
    db::migrate(&reopened).await.unwrap();
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE username = 'existing'")
        .fetch_one(&reopened)
        .await
        .unwrap();
    assert_eq!(name, "Existing User");
    let retained: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = 20261006000000)",
    )
    .fetch_one(&reopened)
    .await
    .unwrap();
    assert!(retained);
}

#[tokio::test]
async fn unknown_missing_versions_and_changed_checksums_still_fail() {
    let (pool, _dir) = database().await;
    db::migrate(&pool).await.unwrap();
    sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (1, 'unknown', true, X'00', 0)")
        .execute(&pool).await.unwrap();
    assert!(matches!(
        db::migrate(&pool).await,
        Err(MigrateError::VersionMissing(1))
    ));
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 1")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum = X'00'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        db::migrate(&pool).await,
        Err(MigrateError::VersionMismatch(20261006010000))
    ));
}

#[tokio::test]
async fn events_migration_backfills_existing_applications_once() {
    let (pool, _dir) = database().await;
    for sql in [
        include_str!("../migrations/20261006010000_create_users_and_sessions.sql"),
        include_str!("../migrations/20261006020000_create_roles.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO users (username, name, email, password_hash) VALUES ('old', 'Old', 'old@example.com', 'unused')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO roles (role_title, company, user_id, created_at) VALUES ('Engineer', 'Example', 1, '2026-10-01T12:34:00.000Z')").execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20261007000000_create_events.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let events: Vec<(String, String)> = sqlx::query_as("SELECT title, occurs_at FROM events")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(
        events,
        vec![("Applied".into(), "2026-10-01T12:34:00.000Z".into())]
    );
}

#[tokio::test]
async fn structured_people_migration_preserves_legacy_contact_details() {
    let (pool, _dir) = database().await;
    for sql in [
        include_str!("../migrations/20261006010000_create_users_and_sessions.sql"),
        include_str!("../migrations/20261006020000_create_roles.sql"),
        include_str!("../migrations/20261007000000_create_events.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO users (username, name, email, password_hash) VALUES ('old', 'Old', 'old@example.com', 'unused')").execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO roles (role_title, company, user_id) VALUES ('Engineer', 'Example', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO event_people (event_id, name, job_title, email) VALUES (1, 'Mary Jane Smith', 'Hiring manager', 'mary@example.com')").execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20261007010000_structure_event_people.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let person: (String, String, String, String, String) =
        sqlx::query_as("SELECT first_name, last_name, email, phone, notes FROM event_people")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        person,
        (
            "Mary Jane Smith".into(),
            "".into(),
            "mary@example.com".into(),
            "".into(),
            "Hiring manager".into()
        )
    );
}
