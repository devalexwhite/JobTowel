use jobtowel::{config::Config, db};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;
    pool.close().await;
    println!("SQLite migrations applied successfully.");
    Ok(())
}
