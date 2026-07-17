use std::{collections::HashMap, env, net::SocketAddr};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub api_addr: SocketAddr,
    pub database_url: String,
    pub max_database_connections: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required configuration: {0}")]
    Missing(&'static str),

    #[error("invalid configuration for {key}: {value}")]
    Invalid { key: &'static str, value: String },
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let values = env::vars().collect::<HashMap<_, _>>();
        Self::from_values(&values)
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let api_addr = values
            .get("RIICHI_API_ADDR")
            .map(String::as_str)
            .unwrap_or("127.0.0.1:3000")
            .parse()
            .map_err(|_| ConfigError::Invalid {
                key: "RIICHI_API_ADDR",
                value: values
                    .get("RIICHI_API_ADDR")
                    .cloned()
                    .unwrap_or_else(|| "127.0.0.1:3000".to_owned()),
            })?;
        let database_url = values
            .get("RIICHI_DATABASE_URL")
            .cloned()
            .ok_or(ConfigError::Missing("RIICHI_DATABASE_URL"))?;
        let max_database_connections = values
            .get("RIICHI_DATABASE_MAX_CONNECTIONS")
            .map(String::as_str)
            .unwrap_or("10")
            .parse()
            .map_err(|_| ConfigError::Invalid {
                key: "RIICHI_DATABASE_MAX_CONNECTIONS",
                value: values
                    .get("RIICHI_DATABASE_MAX_CONNECTIONS")
                    .cloned()
                    .unwrap_or_else(|| "10".to_owned()),
            })?;
        if max_database_connections == 0 {
            return Err(ConfigError::Invalid {
                key: "RIICHI_DATABASE_MAX_CONNECTIONS",
                value: "0".to_owned(),
            });
        }

        Ok(Self {
            api_addr,
            database_url,
            max_database_connections,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(database_url: &str) -> HashMap<String, String> {
        HashMap::from([(String::from("RIICHI_DATABASE_URL"), database_url.to_owned())])
    }

    #[test]
    fn uses_safe_local_defaults() {
        let config = AppConfig::from_values(&values("postgres://localhost/riichi")).unwrap();

        assert_eq!(config.api_addr, "127.0.0.1:3000".parse().unwrap());
        assert_eq!(config.max_database_connections, 10);
    }

    #[test]
    fn rejects_missing_database_url() {
        let error = AppConfig::from_values(&HashMap::new()).unwrap_err();

        assert_eq!(error, ConfigError::Missing("RIICHI_DATABASE_URL"));
    }

    #[test]
    fn rejects_zero_database_connections() {
        let mut values = values("postgres://localhost/riichi");
        values.insert("RIICHI_DATABASE_MAX_CONNECTIONS".to_owned(), "0".to_owned());

        assert!(matches!(
            AppConfig::from_values(&values),
            Err(ConfigError::Invalid {
                key: "RIICHI_DATABASE_MAX_CONNECTIONS",
                ..
            })
        ));
    }
}
