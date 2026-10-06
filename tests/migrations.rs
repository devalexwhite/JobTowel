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
    assert_eq!(tables, ["_sqlx_migrations", "sessions", "users"]);
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
