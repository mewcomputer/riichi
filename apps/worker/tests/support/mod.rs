use std::{ops::Deref, sync::Arc};

use riichi_persistence::Database;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};

#[derive(Clone)]
pub struct PostgresHarness {
    _container: Arc<ContainerAsync<Postgres>>,
    pub database: Database,
}

impl Deref for PostgresHarness {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        &self.database
    }
}

impl PostgresHarness {
    pub async fn start() -> Self {
        let container = Postgres::default()
            .with_host_auth()
            .start()
            .await
            .expect("Docker must be available for worker tests");
        let host = container
            .get_host()
            .await
            .expect("the PostgreSQL container should expose a host");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("the PostgreSQL container should expose port 5432");
        let database = Database::connect(&format!("postgres://postgres@{host}:{port}/postgres"), 5)
            .await
            .expect("the test database should accept connections");
        database
            .migrate()
            .await
            .expect("the test database migrations should apply");

        Self {
            _container: Arc::new(container),
            database,
        }
    }
}
