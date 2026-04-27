use std::collections::BTreeMap;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::types::ModelVisibleToolCatalogSnapshot;

pub(super) fn catalog_hash_payload(snapshot: &ModelVisibleToolCatalogSnapshot) -> Value {
    json!({
        "schema_version": snapshot.schema_version,
        "provider_dialect": snapshot.provider_dialect.as_str(),
        "provider_kind": snapshot.provider_kind,
        "provider_model_id": snapshot.provider_model_id,
        "surface": snapshot.surface.as_str(),
        "principal_hash": snapshot.principal_hash,
        "channel_hash": snapshot.channel_hash,
        "remaining_tool_budget": snapshot.remaining_tool_budget,
        "created_at_unix_ms": snapshot.created_at_unix_ms,
        "tools": snapshot.tools,
        "filtered_tools": snapshot.filtered_tools,
    })
}

pub(super) fn stable_hash_value(value: &Value) -> String {
    stable_hash_bytes(canonical_json_bytes(value).as_slice())
}

pub(super) fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub(super) fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    let mut sorted = value.clone();
    sort_json_value(&mut sorted);
    serde_json::to_vec(&sorted).unwrap_or_else(|_| b"null".to_vec())
}

pub(super) fn sort_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, mut value) in std::mem::take(map) {
                sort_json_value(&mut value);
                sorted.insert(key, value);
            }
            *map = sorted.into_iter().collect();
        }
        Value::Array(values) => {
            for value in values {
                sort_json_value(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
