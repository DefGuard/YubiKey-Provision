use std::{str::Utf8Error, string::FromUtf8Error};

use thiserror::Error;
use tonic::Status;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("Invalid config file. Error: {0}")]
    InvalidConfigFile(String),
    #[error("Tonic Error: {0}")]
    TonicError(String),
    #[error("Tonic request failed with status code of {0}")]
    TonicStatusError(String),
    #[error("GPG command failed")]
    Gpg,
    #[error("Failed to clean up gpg session")]
    GPGSessionEnd,
    #[error("ykman command failed")]
    YubikeyManager,
    #[error("No YubiKeys found")]
    NoKeysFound,
    #[error("Multiple yubikeys found")]
    MultipleKeysPresent,
    #[error("IO error occurred")]
    IO,
    #[error("UTF8 conversion failed")]
    UTF8Conversion,
    #[error("Cannot find key serial number")]
    SerialNotFound,
}

impl From<tonic::transport::Error> for WorkerError {
    fn from(value: tonic::transport::Error) -> Self {
        WorkerError::TonicError(value.to_string())
    }
}

impl From<Status> for WorkerError {
    fn from(value: Status) -> Self {
        WorkerError::TonicStatusError(value.to_string())
    }
}

impl From<std::io::Error> for WorkerError {
    fn from(_value: std::io::Error) -> Self {
        WorkerError::IO
    }
}

impl From<Utf8Error> for WorkerError {
    fn from(_value: Utf8Error) -> Self {
        WorkerError::UTF8Conversion
    }
}

impl From<FromUtf8Error> for WorkerError {
    fn from(_value: FromUtf8Error) -> Self {
        WorkerError::UTF8Conversion
    }
}
