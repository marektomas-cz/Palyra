use std::{net::IpAddr, sync::Arc, time::Duration};

use crate::{
    ca::{CertificateAuthority, IssuedCertificate},
    error::{IdentityError, IdentityResult},
    store::{InMemorySecretStore, SecretStore},
    DEFAULT_CERT_VALIDITY, DEFAULT_PAIRING_WINDOW, DEFAULT_ROTATION_THRESHOLD,
};

use super::{persistence, IdentityManager};

const PENDING_PAIRING_PRIVATE_KEY_KEY_PREFIX: &str = "identity/pairing/pending_private_keys";

impl IdentityManager {
    pub fn with_store(store: Arc<dyn SecretStore>) -> IdentityResult<Self> {
        let (state, loaded_from_bundle) = persistence::load_identity_state(store.as_ref())?;
        let ca = CertificateAuthority::from_stored(&state.ca)?;

        let mut manager = Self {
            store,
            pairing_window: DEFAULT_PAIRING_WINDOW,
            pairing_start_rate_limit_window: persistence::DEFAULT_PAIRING_START_RATE_LIMIT_WINDOW,
            pairing_max_starts_per_window: persistence::DEFAULT_PAIRING_MAX_STARTS_PER_WINDOW,
            recent_pairing_starts: std::collections::VecDeque::new(),
            certificate_validity: DEFAULT_CERT_VALIDITY,
            rotation_threshold: DEFAULT_ROTATION_THRESHOLD,
            active_sessions: std::collections::HashMap::new(),
            paired_devices: state.paired_devices,
            revoked_devices: state.revoked_devices,
            revoked_certificate_fingerprints: state.revoked_certificate_fingerprints,
            state_generation: state.generation,
            ca,
        };
        if !loaded_from_bundle {
            manager.persist_identity_state_bundle()?;
        }

        Ok(manager)
    }

    pub fn with_memory_store() -> IdentityResult<Self> {
        Self::with_store(Arc::new(InMemorySecretStore::new()))
    }

    pub fn set_pairing_window(&mut self, value: Duration) {
        self.pairing_window = value;
    }

    pub fn set_pairing_start_rate_limit(&mut self, max_starts_per_window: usize, window: Duration) {
        self.pairing_max_starts_per_window = max_starts_per_window.max(1);
        self.pairing_start_rate_limit_window =
            if window.is_zero() { Duration::from_millis(1) } else { window };
    }

    pub fn set_certificate_validity(&mut self, value: Duration) {
        self.certificate_validity = value;
    }

    pub fn set_rotation_threshold(&mut self, value: Duration) {
        self.rotation_threshold = value;
    }

    #[must_use]
    pub fn gateway_ca_certificate_pem(&self) -> String {
        self.ca.certificate_pem.clone()
    }

    pub fn issue_gateway_server_certificate(
        &mut self,
        common_name: &str,
    ) -> IdentityResult<IssuedCertificate> {
        self.issue_gateway_server_certificate_with_sans(common_name, &[], &[])
    }

    pub fn issue_gateway_server_certificate_with_sans(
        &mut self,
        common_name: &str,
        additional_dns_names: &[String],
        additional_ip_addresses: &[IpAddr],
    ) -> IdentityResult<IssuedCertificate> {
        self.mutate_persisted_state(|manager| {
            manager.ca.issue_server_certificate_with_sans(
                common_name,
                manager.certificate_validity,
                additional_dns_names,
                additional_ip_addresses,
            )
        })
    }

    pub fn persist_pending_pairing_private_key(
        &self,
        request_id: &str,
        private_key_pem: &str,
    ) -> IdentityResult<()> {
        let key = pending_pairing_private_key_store_key(request_id)?;
        self.store.write_sealed_value(key.as_str(), private_key_pem.as_bytes())
    }

    pub fn load_pending_pairing_private_key(
        &self,
        request_id: &str,
    ) -> IdentityResult<Option<String>> {
        let key = pending_pairing_private_key_store_key(request_id)?;
        match self.store.read_secret(key.as_str()) {
            Ok(raw) => String::from_utf8(raw)
                .map(Some)
                .map_err(|error| IdentityError::Internal(error.to_string())),
            Err(IdentityError::SecretNotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn delete_pending_pairing_private_key(&self, request_id: &str) -> IdentityResult<()> {
        let key = pending_pairing_private_key_store_key(request_id)?;
        self.store.delete_secret(key.as_str())
    }
}

fn pending_pairing_private_key_store_key(request_id: &str) -> IdentityResult<String> {
    let trimmed = request_id.trim();
    if trimmed.is_empty() {
        return Err(IdentityError::InvalidSecretStoreKey);
    }
    Ok(format!("{PENDING_PAIRING_PRIVATE_KEY_KEY_PREFIX}/{}.pem", hex::encode(trimmed.as_bytes())))
}
