use serde::{Deserialize, Serialize};

pub const CONTROL_PLANE_CONTRACT_VERSION: &str = "control-plane.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInfo {
    pub limit: usize,
    pub returned: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractDescriptor {
    pub contract_version: String,
}
