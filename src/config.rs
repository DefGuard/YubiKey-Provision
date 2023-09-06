use std::fs;

use clap::Parser;
use serde::Deserialize;
use toml;

use crate::error::WorkerError;

#[derive(Debug, Parser, Clone, Deserialize)]
#[clap(about = "Defguard YubiKey Provisioning service")]
pub struct Config {
    /// Worker id
    #[arg(long, env = "WORKER_ID", default_value = "YubiBridge")]
    pub worker_id: String,

    /// Logging level
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Url of your DefGuard instance
    #[arg(long, env = "URL", default_value = "localhost:50055")]
    pub url: String,

    /// Number of seconds in which worker pings DefGuard for data provision
    #[arg(long, env = "JOB_INTERVAL", default_value = "2")]
    pub job_interval: u64,

    /// Number of retries in case if there are no smartcards
    #[arg(long, env = "SMARTCARD_RETRIES", default_value = "1")]
    pub smartcard_retries: u64,

    /// Number of seconds before checking for smartcard again
    #[arg(long, env = "SMARTCARD_RETRY_INTERVAL", default_value = "15")]
    pub smartcard_retry_interval: u64,

    /// Token from Defguard available on Provisioning page
    #[arg(
        long,
        short = 't',
        required_unless_present = "config_path",
        env = "DEFGUARD_TOKEN",
        default_value = ""
    )]
    pub token: String,

    /// Configuration file path
    #[arg(long = "config", short)]
    config_path: Option<std::path::PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            worker_id: "YubiBridge".into(),
            log_level: "INFO".into(),
            url: "localhost:50055".into(),
            job_interval: 2,
            smartcard_retries: 1,
            smartcard_retry_interval: 15,
            token: "TOKEN".into(),
            config_path: None,
        }
    }
}

pub fn get_config() -> Result<Config, WorkerError> {
    // parse CLI arguments to get config file path
    let cli_config = Config::parse();

    // load config from file if one was specified
    if let Some(config_path) = cli_config.config_path {
        let config_toml = fs::read_to_string(config_path)
            .map_err(|err| WorkerError::InvalidConfigFile(err.to_string()))?;
        let file_config: Config = toml::from_str(&config_toml)
            .map_err(|err| WorkerError::InvalidConfigFile(err.message().to_string()))?;
        return Ok(file_config);
    }

    Ok(cli_config)
}