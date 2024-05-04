use std::env;
use std::env::current_dir;
use std::fmt::Display;

use config::Config;
use config::ConfigError;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::PgConnectOptions;

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
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
}

/// Database configuration
#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,

    /// Hardcoded to localhost
    pub host: String,

    /// Port for the postgres database, which will be different from that of the
    /// server. Currently hardcoded to 5432.
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub database_name: String,

    /// Should be `true` in production.
    /// https://www.postgresql.org/docs/current/libpq-ssl.html#LIBPQ-SSL-SSLMODE-STATEMENTS
    pub require_ssl: bool,
}

impl DatabaseSettings {
    /// Return connection to a named database (declared in config file). The db
    /// password is concealed.
    pub fn connection(&self) -> PgConnectOptions {
        self.connection_without_db().database(&self.database_name)
        // this appears in the book, but not in the repo
        // https://github.com/LukeMathWalker/zero-to-production/issues/231
        // .log_statements(tracing_log::log::LevelFilter::Trace)
    }

    /// Return connection to the Postgres instance (instead of a specific db),
    /// i.e. `database_name` is unset. This is typically used to init a
    /// randomised db for testing.
    pub fn connection_without_db(&self) -> PgConnectOptions {
        // Secret::new(format!(
        //     "postgres://{}:{}@{}:{}",
        //     self.username,
        //     self.password.expose_secret(),
        //     self.host,
        //     self.port,
        // ));
        PgConnectOptions::new()
            .username(&self.username)
            .password(self.password.expose_secret())
            .host(&self.host)
            .port(self.port)
            // digitalocean's default `sslmode` is `require`, which means: "I want my data to be
            // encrypted, and I accept the overhead. I trust that the network will make sure I
            // always connect to the server I want."
            //
            // https://www.postgresql.org/docs/current/libpq-ssl.html#LIBPQ-SSL-SSLMODE-STATEMENTS
            //
            // so we should adhere to this, and provide transport level encryption
            .ssl_mode(match self.require_ssl {
                true => sqlx::postgres::PgSslMode::Require,
                false => sqlx::postgres::PgSslMode::Prefer,
            })
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

/// Load yaml configuration files at `<project_root>/configuration`.
///
/// All fields must be present in these files, otherwise initialisation will
/// fail immediately, and the server will not start. Invalid configuration is
/// not yet checked.
pub fn get_configuration() -> Result<Settings, ConfigError> {
    let cfg_dir = current_dir()
        .expect("could not get current dir")
        .join("configuration");

    let env: Environment = env::var("APP_ENVIRONMENT")
        .unwrap_or("local".to_string())
        .try_into()
        .expect("could not initiate Environment struct");

    print!("loading config for {env} env");

    let settings = Config::builder()
        // // naive single-file config
        // .add_source(config::File::new(
        //     "configuration.yaml",
        //     config::FileFormat::Yaml,
        // ))
        .add_source(config::File::from(cfg_dir.join("base.yaml")))
        .add_source(config::File::from(cfg_dir.join(format!("{env}.yaml"))))
        .add_source(
            // source env vars, which can be (re?)loaded at runtime, avoiding recompilation. note:
            // env vars are -always- parsed as String, `serde-aux` is required to parse other
            // types.
            //
            // these env vars are to be declared in spec.yaml (under services:envs):
            //
            // `APP_APPLICATION__PORT=5001` -> `Settings.application.port`
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}
