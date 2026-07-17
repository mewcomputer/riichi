use riichi_persistence::Database;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("RIICHI_DATABASE_URL")
        .or_else(|_| env::var("TEST_DATABASE_URL"))
        .map_err(|_| "RIICHI_DATABASE_URL or TEST_DATABASE_URL must be set")?;
    let max_connections = env::var("RIICHI_DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5);

    let database = Database::connect(&database_url, max_connections).await?;
    database.migrate().await?;
    Ok(())
}
