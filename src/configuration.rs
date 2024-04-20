use config::Config;
use config::ConfigError;
use config::FileFormat;
use serde::Deserialize;

/// Server configuration
#[derive(Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    /// Port for the server
    pub application_port: u16,
}

/// Database configuration
#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: String,
    /// Port for the postgres database. This will be different from that of the
    /// server.
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

impl DatabaseSettings {
    /// Return string representation of the database connection
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database_name,
        )
    }
}

/// Loads hardcoded yaml configuration at configuration.yaml. All fields must be
/// present in this file, otherwise initialisation will fail immediately, and
/// the server will not start. Invalid configuration is not yet checked.
pub fn get_configuration() -> Result<Settings, ConfigError> {
    let settings = Config::builder()
        .add_source(config::File::new("configuration.yaml", FileFormat::Yaml))
        .build()?;
    settings.try_deserialize()
}
