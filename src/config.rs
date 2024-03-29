use std::{fs, path::PathBuf};

use clap::Parser;
use serde::Deserialize;

use crate::error::WorkerError;

#[derive(Debug, Parser, Clone, Deserialize)]
#[clap(about = "Defguard YubiKey Provisioning service")]
pub struct Config {
    // Path to CA Used for GRPC connection
    #[arg(long = "ca-file", env = "GRPC_CA")]
    pub grpc_ca: Option<PathBuf>,
    /// ID, this will be also displayed in defguard UI
    #[arg(long = "id", env = "ID", default_value = "YubikeyProvisioner")]
    pub worker_id: String,

    /// Logging level, needs to be compatible with log crate log::LevelFilter::from_str
    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Url of your DefGuard GRPC server
    #[arg(
        long = "grpc",
        env = "GRPC_URL",
        default_value = "http://127.0.0.1:50055"
    )]
    pub url: String,

    /// Number of retries in case if there are no keys detected
    #[arg(long, env = "YUBIKEY_RETRIES", default_value = "1")]
    pub smartcard_retries: u64,

    /// Number of seconds before checking for key again
    #[arg(long, env = "YUBIKEY_RETRY_INTERVAL", default_value = "15")]
    pub smartcard_retry_interval: u64,

    /// gpg debug level, this is set to advanced when log_level is set to debug
    #[arg(long, env = "GPG_DEBUG_LEVEL", default_value = "none")]
    pub gpg_debug_level: String,

    /// Token from Defguard available on Provisioning page
    #[arg(
        long,
        short = 't',
        required_unless_present = "config_path",
        env = "DEFGUARD_TOKEN",
        default_value = ""
    )]
    pub token: String,

    #[arg(
        long = "skip-permissions",
        env = "SKIP_GPG_PERMISSIONS",
        default_value_t = false
    )]
    pub skip_gpg_permissions: bool,

    /// Configuration file path
    #[arg(long = "config", short)]
    config_path: Option<std::path::PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            worker_id: "YubiBridge".into(),
            log_level: "INFO".into(),
            url: "http://127.0.0.1:50055".into(),
            smartcard_retries: 1,
            smartcard_retry_interval: 15,
            token: "TOKEN".into(),
            config_path: None,
            grpc_ca: None,
            skip_gpg_permissions: false,
            gpg_debug_level: "none".into(),
        }
    }
}

pub fn get_config() -> Result<Config, WorkerError> {
    // parse CLI arguments to get config file path
    let mut cli_config = Config::parse();

    // load config from file if one was specified
    if let Some(config_path) = cli_config.config_path {
        let config_toml = fs::read_to_string(config_path)
            .map_err(|err| WorkerError::InvalidConfigFile(err.to_string()))?;
        let file_config: Config = toml::from_str(&config_toml)
            .map_err(|err| WorkerError::InvalidConfigFile(err.message().to_string()))?;
        cli_config = file_config.clone();
    }

    if cli_config.log_level == "debug" && cli_config.gpg_debug_level == "none" {
        cli_config.gpg_debug_level = "advanced".into();
    }

    Ok(cli_config)
}
