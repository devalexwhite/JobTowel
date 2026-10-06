use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{str::FromStr, time::Duration};

// The architecture demo was explicitly retired; its history may remain in local databases.
const RETIRED_DEMO_VERSION: i64 = 20261006000000;
static MIGRATOR: Migrator = {
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.ignore_missing = true;
    migrator
};

pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    let has_history: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if has_history {
        let versions: Vec<i64> = sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?;
        for version in versions {
            if version != RETIRED_DEMO_VERSION
                && !MIGRATOR
                    .iter()
                    .any(|migration| migration.version == version)
            {
                return Err(sqlx::migrate::MigrateError::VersionMissing(version));
            }
        }
    }
    // SQLx still checks dirty migrations and the checksums of every active version.
    MIGRATOR.run(pool).await
}
