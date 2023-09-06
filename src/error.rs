use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("Invalid config file. Error: {0}")]
    InvalidConfigFile(String),
}
