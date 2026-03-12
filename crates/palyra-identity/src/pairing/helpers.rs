use std::time::{Duration, SystemTime};

use hkdf::Hkdf;
use rustls::pki_types::{pem::PemObject, CertificateDer};
use sha2::{Digest, Sha256};

use crate::{
    ca::IssuedCertificate,
    error::{IdentityError, IdentityResult},
    unix_ms,
};

use super::{PairingClientKind, PairingMethod};

pub fn should_rotate_certificate(
    certificate: &IssuedCertificate,
    now: SystemTime,
    threshold: Duration,
) -> IdentityResult<bool> {
    let now_ms = unix_ms(now)?;
    let threshold_ms: u64 = threshold
        .as_millis()
        .try_into()
        .map_err(|_| IdentityError::Internal("rotation threshold overflow".to_owned()))?;
    Ok(certificate.expires_at_unix_ms <= now_ms.saturating_add(threshold_ms))
}

pub(super) fn duration_to_millis_u64(value: Duration) -> u64 {
    value.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(super) fn validate_pairing_method(method: &PairingMethod) -> IdentityResult<()> {
    match method {
        PairingMethod::Pin { code } => {
            let valid = code.len() == 6 && code.chars().all(|ch| ch.is_ascii_digit());
            if !valid {
                return Err(IdentityError::InvalidPairingProof);
            }
        }
        PairingMethod::Qr { token } => {
            if token.len() < 16 || token.len() > 128 {
                return Err(IdentityError::InvalidPairingProof);
            }
        }
    }
    Ok(())
}

pub(super) fn pairing_signature_payload(
    protocol_version: u32,
    session_id: &str,
    challenge: &[u8; 32],
    gateway_ephemeral_public: &[u8; 32],
    device_id: &str,
    client_kind: PairingClientKind,
    proof: &str,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(256);
    payload.extend_from_slice(b"palyra-pairing-v1");
    payload.extend_from_slice(&protocol_version.to_le_bytes());
    payload.extend_from_slice(session_id.as_bytes());
    payload.extend_from_slice(challenge);
    payload.extend_from_slice(gateway_ephemeral_public);
    payload.extend_from_slice(device_id.as_bytes());
    payload.extend_from_slice(client_kind.as_str().as_bytes());
    payload.extend_from_slice(proof.as_bytes());
    payload
}

pub(super) fn transcript_context(
    session_id: &str,
    protocol_version: u32,
    device_id: &str,
    client_kind: PairingClientKind,
) -> Vec<u8> {
    let mut context = Vec::with_capacity(128);
    context.extend_from_slice(b"palyra-mtls-transcript-v1");
    context.extend_from_slice(session_id.as_bytes());
    context.extend_from_slice(&protocol_version.to_le_bytes());
    context.extend_from_slice(device_id.as_bytes());
    context.extend_from_slice(client_kind.as_str().as_bytes());
    context
}

pub(super) fn derive_transcript_mac(
    shared_secret: &[u8; 32],
    challenge: &[u8; 32],
    transcript_context: &[u8],
) -> IdentityResult<[u8; 32]> {
    let hkdf = Hkdf::<Sha256>::new(Some(challenge), shared_secret);
    let mut output = [0_u8; 32];
    hkdf.expand(transcript_context, &mut output)
        .map_err(|_| IdentityError::Cryptographic("hkdf expansion failed".to_owned()))?;
    Ok(output)
}

pub(super) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    let max_len = left.len().max(right.len());

    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        diff |= usize::from(left_byte ^ right_byte);
    }

    diff == 0
}

pub(super) fn certificate_fingerprint_hex(certificate_pem: &str) -> IdentityResult<String> {
    let der = CertificateDer::from_pem_slice(certificate_pem.as_bytes())
        .map_err(|_| IdentityError::CertificateParsingFailed)?;
    Ok(hex::encode(Sha256::digest(der.as_ref())))
}
