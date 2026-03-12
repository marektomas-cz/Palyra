use std::path::{Path, PathBuf};

use anyhow::Context;
use palyra_common::default_identity_store_root;

use crate::{
    backend::{select_backend, BackendKind, BackendPreference, BlobBackend},
    crypto::{build_aad, derive_device_kek, validate_secret_key},
    envelope::{open, seal, EnvelopePayload},
    filesystem::{default_vault_root, ensure_owner_only_dir, normalize_vault_root_path},
    metadata::{self, MetadataEntry, MetadataFile, MetadataLockGuard},
    scope::VaultScope,
};

const DEFAULT_MAX_SECRET_BYTES: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("secret not found")]
    NotFound,
    #[error("invalid scope: {0}")]
    InvalidScope(String),
    #[error("invalid secret key: {0}")]
    InvalidKey(String),
    #[error("invalid object id: {0}")]
    InvalidObjectId(String),
    #[error("secret value exceeds max bytes ({actual} > {max})")]
    ValueTooLarge { actual: usize, max: usize },
    #[error("vault backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("vault crypto failure: {0}")]
    Crypto(String),
    #[error("vault I/O failure: {0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMetadata {
    pub scope: VaultScope,
    pub key: String,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
    pub value_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultRef {
    pub scope: VaultScope,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct VaultConfig {
    pub root: Option<PathBuf>,
    pub identity_store_root: Option<PathBuf>,
    pub backend_preference: BackendPreference,
    pub max_secret_bytes: usize,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            root: None,
            identity_store_root: None,
            backend_preference: BackendPreference::Auto,
            max_secret_bytes: DEFAULT_MAX_SECRET_BYTES,
        }
    }
}

pub struct Vault {
    pub(crate) root: PathBuf,
    pub(crate) backend: Box<dyn BlobBackend>,
    pub(crate) max_secret_bytes: usize,
    pub(crate) kek: [u8; 32],
}

impl Vault {
    pub fn open_default() -> Result<Self, VaultError> {
        Self::open_with_config(VaultConfig::default())
    }

    pub fn open_with_config(config: VaultConfig) -> Result<Self, VaultError> {
        let identity_store_root = if let Some(path) = config.identity_store_root {
            path
        } else {
            default_identity_store_root()
                .context("failed to resolve default identity store root")
                .map_err(|error| VaultError::Io(error.to_string()))?
        };
        let root_raw = if let Some(path) = config.root {
            path
        } else if let Ok(path) = std::env::var("PALYRA_VAULT_DIR") {
            PathBuf::from(path)
        } else {
            default_vault_root(identity_store_root.as_path())
        };
        let root = normalize_vault_root_path(root_raw)?;
        if config.max_secret_bytes == 0 {
            return Err(VaultError::InvalidKey(
                "max secret bytes must be greater than zero".to_owned(),
            ));
        }

        ensure_owner_only_dir(&root)?;
        let root = crate::canonicalize_existing_dir(root.as_path(), "vault root directory")?;
        let backend = select_backend(&root, config.backend_preference)?;
        let kek = derive_device_kek(identity_store_root.as_path())?;
        let vault = Self { root, backend, max_secret_bytes: config.max_secret_bytes, kek };
        vault.ensure_metadata_exists()?;
        Ok(vault)
    }

    #[must_use]
    pub fn backend_kind(&self) -> BackendKind {
        self.backend.kind()
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put_secret(
        &self,
        scope: &VaultScope,
        key: &str,
        value: &[u8],
    ) -> Result<SecretMetadata, VaultError> {
        validate_secret_key(key)?;
        if value.len() > self.max_secret_bytes {
            return Err(VaultError::ValueTooLarge {
                actual: value.len(),
                max: self.max_secret_bytes,
            });
        }
        let aad = crate::build_aad(scope, key);
        let envelope = seal(value, &self.kek, aad.as_slice())?;
        let payload = serde_json::to_vec(&envelope).map_err(|error| {
            VaultError::Io(format!("failed to serialize envelope payload: {error}"))
        })?;
        let object_id = crate::object_id_for(scope, key);
        let now = crate::current_unix_ms()?;

        let _lock = self.acquire_metadata_lock()?;
        let mut index = self.read_metadata()?;
        let existing_entry_index =
            index.entries.iter().position(|entry| entry.scope == *scope && entry.key == key);
        let previous_blob = if existing_entry_index.is_some() {
            Some(self.backend.get_blob(object_id.as_str())?)
        } else {
            None
        };
        self.backend.put_blob(object_id.as_str(), payload.as_slice())?;

        let entry = if let Some(existing_index) = existing_entry_index {
            let existing = &mut index.entries[existing_index];
            existing.updated_at_unix_ms = now;
            existing.value_bytes = value.len();
            existing.object_id = object_id.clone();
            existing.clone()
        } else {
            let created = MetadataEntry {
                scope: scope.clone(),
                key: key.to_owned(),
                object_id: object_id.clone(),
                created_at_unix_ms: now,
                updated_at_unix_ms: now,
                value_bytes: value.len(),
            };
            index.entries.push(created.clone());
            created
        };
        if let Err(write_error) = self.write_metadata(&index) {
            let rollback_result = if let Some(previous_blob) = previous_blob.as_ref() {
                self.backend.put_blob(object_id.as_str(), previous_blob.as_slice())
            } else {
                self.backend.delete_blob(object_id.as_str())
            };
            if let Err(rollback_error) = rollback_result {
                return Err(VaultError::Io(format!(
                    "failed to persist metadata after blob write and failed to rollback blob: write_error={write_error}; rollback_error={rollback_error}"
                )));
            }
            return Err(VaultError::Io(format!(
                "failed to persist metadata after blob write: {write_error}"
            )));
        }
        Ok(entry.into())
    }

    pub fn get_secret(&self, scope: &VaultScope, key: &str) -> Result<Vec<u8>, VaultError> {
        validate_secret_key(key)?;
        let _lock = self.acquire_metadata_lock()?;
        let index = self.read_metadata()?;
        let entry = index
            .entries
            .iter()
            .find(|entry| entry.scope == *scope && entry.key == key)
            .cloned()
            .ok_or(VaultError::NotFound)?;
        let payload = self.backend.get_blob(entry.object_id.as_str())?;
        let envelope: EnvelopePayload =
            serde_json::from_slice(payload.as_slice()).map_err(|error| {
                VaultError::Crypto(format!("failed to parse envelope payload: {error}"))
            })?;
        let aad = build_aad(scope, key);
        open(&envelope, &self.kek, aad.as_slice())
    }

    pub fn delete_secret(&self, scope: &VaultScope, key: &str) -> Result<bool, VaultError> {
        validate_secret_key(key)?;
        let _lock = self.acquire_metadata_lock()?;
        let mut index = self.read_metadata()?;
        let index_before_delete = index.clone();
        let mut deleted = false;
        let mut removed_object_id = None;
        index.entries.retain(|entry| {
            if entry.scope == *scope && entry.key == key {
                deleted = true;
                removed_object_id = Some(entry.object_id.clone());
                false
            } else {
                true
            }
        });
        if let Some(object_id) = removed_object_id {
            self.write_metadata(&index)?;
            if let Err(error) = self.backend.delete_blob(object_id.as_str()) {
                self.write_metadata(&index_before_delete).map_err(|rollback_error| {
                    VaultError::Io(format!(
                        "failed to delete secret blob and rollback metadata: delete_error={error}; rollback_error={rollback_error}"
                    ))
                })?;
                return Err(error);
            }
        }
        Ok(deleted)
    }

    pub fn list_secrets(&self, scope: &VaultScope) -> Result<Vec<SecretMetadata>, VaultError> {
        let _lock = self.acquire_metadata_lock()?;
        let index = self.read_metadata()?;
        let mut results = index
            .entries
            .iter()
            .filter(|entry| entry.scope == *scope)
            .cloned()
            .map(SecretMetadata::from)
            .collect::<Vec<_>>();
        results.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(results)
    }

    pub(crate) fn ensure_metadata_exists(&self) -> Result<(), VaultError> {
        metadata::ensure_metadata_exists(self.root.as_path())
    }

    pub(crate) fn acquire_metadata_lock(&self) -> Result<MetadataLockGuard, VaultError> {
        metadata::acquire_metadata_lock(self.root.as_path())
    }

    pub(crate) fn read_metadata(&self) -> Result<MetadataFile, VaultError> {
        metadata::read_metadata(self.root.as_path())
    }

    pub(crate) fn write_metadata(&self, metadata: &MetadataFile) -> Result<(), VaultError> {
        metadata::write_metadata(self.root.as_path(), metadata)
    }
}

impl VaultRef {
    pub fn parse(raw: &str) -> Result<Self, VaultError> {
        let normalized = raw.trim();
        let (scope_raw, key_raw) = normalized.split_once('/').ok_or_else(|| {
            VaultError::InvalidKey(
                "vault ref must have shape '<scope>/<key>' (for example 'global/openai_api_key')"
                    .to_owned(),
            )
        })?;
        let scope = scope_raw.parse::<VaultScope>()?;
        validate_secret_key(key_raw)?;
        Ok(Self { scope, key: key_raw.to_owned() })
    }
}

impl From<MetadataEntry> for SecretMetadata {
    fn from(value: MetadataEntry) -> Self {
        Self {
            scope: value.scope,
            key: value.key,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
            value_bytes: value.value_bytes,
        }
    }
}
