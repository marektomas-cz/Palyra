use thiserror::Error;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Auth,
    Validation,
    Policy,
    NotFound,
    Conflict,
    Dependency,
    Availability,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: String,
    pub code: String,
    pub category: ErrorCategory,
    pub retryable: bool,
    #[serde(default)]
    pub redacted: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_errors: Vec<ValidationIssue>,
}

impl ErrorEnvelope {
    #[must_use]
    pub fn message(&self) -> &str {
        self.error.as_str()
    }
}

#[derive(Debug, Error)]
pub enum ControlPlaneClientError {
    #[error("invalid control-plane base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("HTTP client initialization failed: {0}")]
    ClientInit(String),
    #[error("request failed: {0}")]
    Transport(String),
    #[error("request failed with HTTP {status}: {message}")]
    Http { status: u16, message: String, envelope: Option<ErrorEnvelope> },
    #[error("response decoding failed: {0}")]
    Decode(String),
}
