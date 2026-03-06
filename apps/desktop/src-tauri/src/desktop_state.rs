use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use palyra_vault::{BackendPreference, Vault, VaultConfig, VaultError, VaultScope};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::{
    normalize_optional_text, DESKTOP_SECRET_KEY_ADMIN_TOKEN, DESKTOP_SECRET_KEY_BROWSER_AUTH_TOKEN,
    DESKTOP_SECRET_MAX_BYTES, DESKTOP_STATE_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DesktopStateFile {
    pub(crate) schema_version: u32,
    pub(crate) browser_service_enabled: bool,
}

impl DesktopStateFile {
    pub(crate) fn new_default() -> Self {
        Self { schema_version: DESKTOP_STATE_SCHEMA_VERSION, browser_service_enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyDesktopStateFile {
    #[serde(default = "default_legacy_schema_version")]
    schema_version: u32,
    #[serde(default)]
    admin_token: String,
    #[serde(default)]
    browser_auth_token: String,
    #[serde(default = "default_browser_service_enabled")]
    browser_service_enabled: bool,
}

impl LegacyDesktopStateFile {
    fn into_state(self) -> DesktopStateFile {
        let _ = self.schema_version;
        DesktopStateFile {
            schema_version: DESKTOP_STATE_SCHEMA_VERSION,
            browser_service_enabled: self.browser_service_enabled,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedDesktopState {
    pub(crate) persisted: DesktopStateFile,
    pub(crate) admin_token: String,
    pub(crate) browser_auth_token: String,
}

pub(crate) struct DesktopSecretStore {
    vault: Vault,
}

impl DesktopSecretStore {
    pub(crate) fn open(state_dir: &Path) -> Result<Self> {
        let backend_preference =
            if cfg!(test) { BackendPreference::EncryptedFile } else { BackendPreference::Auto };
        let vault = Vault::open_with_config(VaultConfig {
            root: Some(state_dir.join("vault")),
            identity_store_root: Some(state_dir.join("identity")),
            backend_preference,
            max_secret_bytes: DESKTOP_SECRET_MAX_BYTES,
        })
        .map_err(|error| anyhow!("failed to initialize desktop secret store: {error}"))?;
        Ok(Self { vault })
    }

    pub(crate) fn load_or_create_secret(
        &self,
        key: &str,
        legacy_value: Option<&str>,
    ) -> Result<String> {
        let scope = VaultScope::Global;
        if let Some(value) = self.read_secret_utf8(&scope, key)? {
            return Ok(value);
        }

        let value = normalize_optional_text(legacy_value.unwrap_or_default())
            .map(ToOwned::to_owned)
            .unwrap_or_else(generate_secret_token);
        self.vault
            .put_secret(&scope, key, value.as_bytes())
            .map_err(|error| anyhow!("failed to persist desktop secret '{key}': {error}"))?;
        Ok(value)
    }

    fn read_secret_utf8(&self, scope: &VaultScope, key: &str) -> Result<Option<String>> {
        match self.vault.get_secret(scope, key) {
            Ok(raw) => {
                let decoded = String::from_utf8(raw)
                    .with_context(|| format!("desktop secret '{key}' contains non UTF-8 bytes"))?;
                if decoded.trim().is_empty() {
                    return Ok(None);
                }
                Ok(Some(decoded))
            }
            Err(VaultError::NotFound) => Ok(None),
            Err(error) => Err(anyhow!("failed to read desktop secret '{key}': {error}")),
        }
    }
}

const fn default_legacy_schema_version() -> u32 {
    1
}

const fn default_browser_service_enabled() -> bool {
    true
}

pub(crate) fn resolve_desktop_state_root() -> Result<PathBuf> {
    palyra_common::default_state_root().map_err(|error| {
        anyhow!("failed to resolve default state root for desktop control center: {}", error)
    })
}

pub(crate) fn load_or_initialize_state_file(
    path: &Path,
    secret_store: &DesktopSecretStore,
) -> Result<LoadedDesktopState> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create desktop state directory {}", parent.display())
        })?;
    }

    if path.exists() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read desktop state file {}", path.display()))?;
        let legacy_state: LegacyDesktopStateFile = serde_json::from_str(raw.as_str())
            .with_context(|| format!("failed to parse desktop state file {}", path.display()))?;
        let admin_token = secret_store.load_or_create_secret(
            DESKTOP_SECRET_KEY_ADMIN_TOKEN,
            Some(legacy_state.admin_token.as_str()),
        )?;
        let browser_auth_token = secret_store.load_or_create_secret(
            DESKTOP_SECRET_KEY_BROWSER_AUTH_TOKEN,
            Some(legacy_state.browser_auth_token.as_str()),
        )?;
        let persisted = legacy_state.into_state();
        persist_desktop_state_file(path, &persisted, "normalized")?;
        return Ok(LoadedDesktopState { persisted, admin_token, browser_auth_token });
    }

    let persisted = DesktopStateFile::new_default();
    let admin_token = secret_store.load_or_create_secret(DESKTOP_SECRET_KEY_ADMIN_TOKEN, None)?;
    let browser_auth_token =
        secret_store.load_or_create_secret(DESKTOP_SECRET_KEY_BROWSER_AUTH_TOKEN, None)?;
    persist_desktop_state_file(path, &persisted, "default")?;
    Ok(LoadedDesktopState { persisted, admin_token, browser_auth_token })
}

fn persist_desktop_state_file(path: &Path, state: &DesktopStateFile, label: &str) -> Result<()> {
    let encoded = serde_json::to_string_pretty(state)
        .with_context(|| format!("failed to encode {label} desktop state file"))?;
    fs::write(path, encoded)
        .with_context(|| format!("failed to persist {label} desktop state file {}", path.display()))
}

fn generate_secret_token() -> String {
    format!("{}{}", Ulid::new(), Ulid::new())
}
