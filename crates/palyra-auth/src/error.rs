use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthProfileError {
    #[error("auth profile registry lock poisoned")]
    LockPoisoned,
    #[error("invalid path in {field}: {message}")]
    InvalidPath { field: &'static str, message: String },
    #[error("failed to read auth profile registry {path}: {source}")]
    ReadRegistry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse auth profile registry {path}: {source}")]
    ParseRegistry {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("failed to write auth profile registry {path}: {source}")]
    WriteRegistry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize auth profile registry: {0}")]
    SerializeRegistry(#[from] toml::ser::Error),
    #[error("unsupported auth profile registry version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid field '{field}': {message}")]
    InvalidField { field: &'static str, message: String },
    #[error("auth profile not found: {0}")]
    ProfileNotFound(String),
    #[error("auth profile registry exceeds maximum entries")]
    RegistryLimitExceeded,
    #[error("system time before unix epoch: {0}")]
    InvalidSystemTime(#[from] std::time::SystemTimeError),
}
