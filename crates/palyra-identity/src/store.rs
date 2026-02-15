use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::error::{IdentityError, IdentityResult};

pub trait SecretStore: Send + Sync {
    fn write_secret(&self, key: &str, value: &[u8]) -> IdentityResult<()>;
    fn read_secret(&self, key: &str) -> IdentityResult<Vec<u8>>;
    fn delete_secret(&self, key: &str) -> IdentityResult<()>;
}

#[derive(Default)]
pub struct InMemorySecretStore {
    state: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemorySecretStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemorySecretStore {
    fn write_secret(&self, key: &str, value: &[u8]) -> IdentityResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| IdentityError::Internal("store lock poisoned".to_owned()))?;
        state.insert(key.to_owned(), value.to_vec());
        Ok(())
    }

    fn read_secret(&self, key: &str) -> IdentityResult<Vec<u8>> {
        let state = self
            .state
            .lock()
            .map_err(|_| IdentityError::Internal("store lock poisoned".to_owned()))?;
        state.get(key).cloned().ok_or(IdentityError::SecretNotFound)
    }

    fn delete_secret(&self, key: &str) -> IdentityResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| IdentityError::Internal("store lock poisoned".to_owned()))?;
        state.remove(key);
        Ok(())
    }
}

pub struct FilesystemSecretStore {
    root: PathBuf,
}

impl FilesystemSecretStore {
    pub fn new(root: impl Into<PathBuf>) -> IdentityResult<Self> {
        let root = root.into();
        #[cfg(windows)]
        {
            let _ = &root;
            Err(IdentityError::Internal(
                "FilesystemSecretStore on Windows is disabled until ACL hardening is implemented"
                    .to_owned(),
            ))
        }
        #[cfg(not(windows))]
        {
            fs::create_dir_all(&root)
                .map_err(|error| IdentityError::Internal(error.to_string()))?;
            Ok(Self { root })
        }
    }

    fn key_path(&self, key: &str) -> IdentityResult<PathBuf> {
        if key.is_empty()
            || key.contains("..")
            || key.contains('\\')
            || key.contains(':')
            || key.starts_with('/')
        {
            return Err(IdentityError::InvalidSecretStoreKey);
        }
        Ok(self.root.join(key.replace('/', "__")))
    }
}

impl SecretStore for FilesystemSecretStore {
    fn write_secret(&self, key: &str, value: &[u8]) -> IdentityResult<()> {
        #[cfg(windows)]
        {
            let _ = key;
            let _ = value;
            Err(IdentityError::Internal(
                "FilesystemSecretStore on Windows is disabled until ACL hardening is implemented"
                    .to_owned(),
            ))
        }
        #[cfg(not(windows))]
        let path = self.key_path(key)?;
        #[cfg(not(windows))]
        fs::write(&path, value).map_err(|error| IdentityError::Internal(error.to_string()))?;
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, permissions)
                .map_err(|error| IdentityError::Internal(error.to_string()))?;
            Ok(())
        }
    }

    fn read_secret(&self, key: &str) -> IdentityResult<Vec<u8>> {
        let path = self.key_path(key)?;
        fs::read(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                IdentityError::SecretNotFound
            } else {
                IdentityError::Internal(error.to_string())
            }
        })
    }

    fn delete_secret(&self, key: &str) -> IdentityResult<()> {
        let path = self.key_path(key)?;
        if path.exists() {
            fs::remove_file(path).map_err(|error| IdentityError::Internal(error.to_string()))?;
        }
        Ok(())
    }
}

pub fn default_identity_storage_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(".palyra").join("identity")
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::{FilesystemSecretStore, SecretStore};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    fn filesystem_secret_store_sets_owner_only_permissions() {
        let temp = tempdir().expect("temp dir should initialize");
        let store = FilesystemSecretStore::new(temp.path()).expect("store should initialize");
        store.write_secret("device/test.json", br#"{"ok":true}"#).expect("secret should persist");
        let path = temp.path().join("device__test.json");
        let metadata = std::fs::metadata(path).expect("metadata should be readable");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
