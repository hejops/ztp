use std::env;
use std::env::current_dir;
use std::fmt::Display;

use config::Config;
use config::ConfigError;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;

/// Global configuration, loaded from configuration.yaml. See
/// `get_configuration`.
#[derive(Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

/// Server configuration
#[derive(Deserialize)]
pub struct ApplicationSettings {
    /// Should be localhost on dev machine, 0.0.0.0 on prod
    pub host: String,
    /// Port for the server, currently hardcoded to 8000
    pub port: u16,
}

/// Database configuration
#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    /// Hardcoded to localhost
    pub host: String,
    /// Port for the postgres database. This will be different from that of the
    /// server, currently hardcoded to 5432.
    pub port: u16,
    pub database_name: String,
}

impl DatabaseSettings {
    /// Return string representation of the database connection. The db password
    /// is concealed.
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.database_name,
        ))
    }

    /// Return string representation of the Postgres instance (instead of a
    /// specific db). This is typically used to init a randomised db for
    /// testing.
    pub fn connection_string_without_db(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
        ))
    }
}

pub enum Environment {
    Local,
    Production,
}

impl Display for Environment {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Environment::Local => "local",
                Environment::Production => "production",
            }
        )?;
        Ok(())
    }
}

impl TryFrom<String> for Environment {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            e => Err(format!("Invalid: {e}")),
        }
    }
}

/// Load yaml configuration files at configuration.
///
/// All fields must be present in these files, otherwise initialisation will
/// fail immediately, and the server will not start. Invalid configuration is
/// not yet checked.
pub fn get_configuration() -> Result<Settings, ConfigError> {
    let cfg_dir = current_dir().unwrap().join("configuration");

    let env: Environment = env::var("APP_ENVIRONMENT")
        .unwrap_or("local".to_string())
        .try_into()
        .unwrap();

    let settings = Config::builder()
        // // naive single-file config
        // .add_source(config::File::new(
        //     "configuration.yaml",
        //     config::FileFormat::Yaml,
        // ))
        .add_source(config::File::from(cfg_dir.join("base.yaml")))
        .add_source(config::File::from(cfg_dir.join(format!("{env}.yaml"))))
        .build()?;

    settings.try_deserialize()
}
