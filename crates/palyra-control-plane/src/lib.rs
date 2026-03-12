mod client;
mod contract;
mod errors;
mod models;
mod transport;

pub use client::{ControlPlaneClient, ControlPlaneClientConfig};
pub use contract::{ContractDescriptor, PageInfo, CONTROL_PLANE_CONTRACT_VERSION};
pub use errors::{ControlPlaneClientError, ErrorCategory, ErrorEnvelope, ValidationIssue};
pub use models::*;

#[cfg(test)]
mod tests;
