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
            "people",
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

#[tokio::test]
async fn active_status_migration_defaults_existing_and_new_applications_to_active() {
    let (pool, _dir) = database().await;
    for sql in [
        include_str!("../migrations/20261006010000_create_users_and_sessions.sql"),
        include_str!("../migrations/20261006020000_create_roles.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO users (username, name, email, password_hash) VALUES ('old', 'Old', 'old@example.com', 'unused')").execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO roles (role_title, company, user_id) VALUES ('Existing', 'Example', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20261007020000_application_active_status.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO roles (role_title, company, user_id) VALUES ('New', 'Example', 1)")
        .execute(&pool)
        .await
        .unwrap();
    let statuses: Vec<bool> = sqlx::query_scalar("SELECT active FROM roles ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(statuses, vec![true, true]);
    assert!(
        sqlx::query("UPDATE roles SET active = 2")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn timezone_migration_preserves_existing_users_and_event_instants() {
    let (pool, _dir) = database().await;
    for sql in [
        include_str!("../migrations/20261006010000_create_users_and_sessions.sql"),
        include_str!("../migrations/20261006020000_create_roles.sql"),
        include_str!("../migrations/20261007000000_create_events.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO users (username, name, email, password_hash) VALUES ('old', 'Old', 'old@example.com', 'unchanged')").execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO roles (role_title, company, user_id) VALUES ('Engineer', 'Example', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before: String = sqlx::query_scalar("SELECT occurs_at FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20261007030000_user_timezone.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let user: (String, Option<String>) =
        sqlx::query_as("SELECT password_hash, timezone FROM users")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(user, ("unchanged".into(), None));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT occurs_at FROM events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn application_people_migration_preserves_links_and_scopes_deduplication() {
    let (pool, _dir) = database().await;
    for sql in [
        include_str!("../migrations/20261006010000_create_users_and_sessions.sql"),
        include_str!("../migrations/20261006020000_create_roles.sql"),
        include_str!("../migrations/20261007000000_create_events.sql"),
        include_str!("../migrations/20261007010000_structure_event_people.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO users (username, name, email, password_hash) VALUES ('old', 'Old', 'old@example.com', 'unused');
        INSERT INTO roles (role_title, company, user_id) VALUES ('One', 'Example', 1), ('Two', 'Example', 1);
        INSERT INTO events (role_id, title, occurs_at) VALUES (1, 'Interview', '2026-10-08T12:00:00.000Z');
        INSERT INTO event_people (event_id, first_name, last_name, email, phone, notes) VALUES
        (1, 'Jo', 'Smith', 'jo@example.com', '+1 555', 'Manager'),
        (3, 'Jo', 'Smith', 'jo@example.com', '+1 555', 'Manager'),
        (2, 'Jo', 'Smith', 'jo@example.com', '+1 555', 'Manager'),
        (3, 'Jo', 'Smith', 'jo@example.com', '+1 555', 'Different notes');")
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20261008000000_application_people.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let people: i64 = sqlx::query_scalar("SELECT count(*) FROM people")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(people, 3);
    let links: Vec<(i64, i64)> =
        sqlx::query_as("SELECT event_id, person_id FROM event_people ORDER BY event_id, person_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(links, vec![(1, 1), (2, 3), (3, 1), (3, 4)]);
    let details: (String, String, String, String, String) = sqlx::query_as(
        "SELECT first_name, last_name, email, phone, notes FROM people WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        details,
        (
            "Jo".into(),
            "Smith".into(),
            "jo@example.com".into(),
            "+1 555".into(),
            "Manager".into()
        )
    );
    sqlx::query("DELETE FROM events WHERE role_id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM people")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    sqlx::raw_sql(include_str!(
        "../migrations/20261008010000_remove_unlinked_people.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM people ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, vec![3]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event_people")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("DELETE FROM roles WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM people")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn person_job_title_migration_preserves_existing_contacts() {
    let (pool, _dir) = database().await;
    for sql in [
        include_str!("../migrations/20261006010000_create_users_and_sessions.sql"),
        include_str!("../migrations/20261006020000_create_roles.sql"),
        include_str!("../migrations/20261007000000_create_events.sql"),
        include_str!("../migrations/20261007010000_structure_event_people.sql"),
        include_str!("../migrations/20261008000000_application_people.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO users (username, name, email, password_hash) VALUES ('old', 'Old', 'old@example.com', 'unused');
        INSERT INTO roles (role_title, company, user_id) VALUES ('Engineer', 'Example', 1);
        INSERT INTO people (role_id, first_name, notes) VALUES (1, 'Jo', 'Existing notes');
        INSERT INTO event_people (event_id, person_id, role_id) VALUES (1, 1, 1);")
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20261008020000_person_job_title.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let contact: (String, String, String) =
        sqlx::query_as("SELECT first_name, job_title, notes FROM people WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(contact, ("Jo".into(), "".into(), "Existing notes".into()));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT person_id FROM event_people WHERE event_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("INSERT INTO people (role_id, first_name) VALUES (1, 'New')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT job_title FROM people WHERE first_name = 'New'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        ""
    );
}

#[tokio::test]
async fn theme_migration_defaults_existing_users_and_rejects_invalid_preferences() {
    let (pool, _dir) = database().await;
    sqlx::raw_sql(include_str!(
        "../migrations/20261006010000_create_users_and_sessions.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO users (username, name, email, password_hash) VALUES ('old', 'Old', 'old@example.com', 'unchanged')").execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("../migrations/20261016000000_user_theme.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let user: (String, String) = sqlx::query_as("SELECT password_hash, theme FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(user, ("unchanged".into(), "auto".into()));
    for theme in ["auto", "light", "dark"] {
        sqlx::query("UPDATE users SET theme = ?")
            .bind(theme)
            .execute(&pool)
            .await
            .unwrap();
    }
    assert!(
        sqlx::query("UPDATE users SET theme = 'invalid'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE users SET theme = NULL")
            .execute(&pool)
            .await
            .is_err()
    );
}
