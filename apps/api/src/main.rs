use riichi_application::{Application, config::AppConfig};
use riichi_auth::{AuthService, OidcConfig};
use riichi_persistence::Database;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = AppConfig::from_env()?;
    let database = Database::connect(&config.database_url, config.max_database_connections).await?;
    database.migrate().await?;
    let auth = AuthService::discover(OidcConfig::from_env()?).await?;
    let listener = TcpListener::bind(config.api_addr).await?;

    info!(addr = %config.api_addr, "riichi api listening");
    axum::serve(
        listener,
        riichi_api::app_with_auth(Application::new(database), auth),
    )
    .await?;

    Ok(())
}
