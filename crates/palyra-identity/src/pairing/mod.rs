use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};

use crate::{ca::CertificateAuthority, store::SecretStore};

mod handshake;
mod helpers;
mod manager;
mod models;
mod persistence;
mod revocation;
#[cfg(test)]
mod tests;

pub use handshake::build_device_pairing_hello;
pub use helpers::should_rotate_certificate;
pub use models::{
    DevicePairingHello, PairedDevice, PairingClientKind, PairingMethod, PairingResult,
    PairingSession, RevokedDevice, VerifiedPairing,
};

pub struct IdentityManager {
    store: Arc<dyn SecretStore>,
    pairing_window: Duration,
    pairing_start_rate_limit_window: Duration,
    pairing_max_starts_per_window: usize,
    recent_pairing_starts: VecDeque<u64>,
    certificate_validity: Duration,
    rotation_threshold: Duration,
    active_sessions: HashMap<String, models::ActivePairingSession>,
    paired_devices: HashMap<String, PairedDevice>,
    revoked_devices: HashMap<String, RevokedDevice>,
    revoked_certificate_fingerprints: HashSet<String>,
    state_generation: u64,
    ca: CertificateAuthority,
}
