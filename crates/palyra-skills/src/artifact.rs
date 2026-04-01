use std::{
    collections::BTreeMap,
    convert::TryInto,
    io::{Cursor, Read, Write},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::constants::{
    MAX_ARTIFACT_BYTES, MAX_ENTRIES, MAX_ENTRY_BYTES, PAYLOAD_CONTEXT, PROVENANCE_PATH, SBOM_PATH,
    SIGNATURE_ALGORITHM, SIGNATURE_PATH, SKILL_MANIFEST_PATH,
};
use crate::error::SkillPackagingError;
use crate::manifest::{
    assert_runtime_compatibility, parse_manifest_toml, validate_provenance_payload,
    validate_sbom_payload,
};
use crate::models::{
    ParsedArtifact, SkillArtifactBuildOutput, SkillArtifactBuildRequest, SkillArtifactSignature,
    SkillIntegrityEntry,
};

pub fn build_signed_skill_artifact(
    request: SkillArtifactBuildRequest,
) -> Result<SkillArtifactBuildOutput, SkillPackagingError> {
    let mut manifest = parse_manifest_toml(request.manifest_toml.as_str())?;
    assert_runtime_compatibility(&manifest.compat)?;
    validate_sbom_payload(request.sbom_cyclonedx_json.as_slice())?;
    validate_provenance_payload(request.provenance_json.as_slice())?;

    let mut payload_entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for module in request.modules {
        if !module.path.ends_with(".wasm") {
            return Err(SkillPackagingError::ManifestValidation(format!(
                "module path '{}' must end with .wasm",
                module.path
            )));
        }
        insert_entry(
            &mut payload_entries,
            format!("modules/{}", normalize_artifact_path(module.path.as_str())?).as_str(),
            module.bytes,
        )?;
    }
    if payload_entries.is_empty() {
        return Err(SkillPackagingError::ManifestValidation(
            "artifact must include at least one module".to_owned(),
        ));
    }

    for asset in request.assets {
        insert_entry(
            &mut payload_entries,
            format!("assets/{}", normalize_artifact_path(asset.path.as_str())?).as_str(),
            asset.bytes,
        )?;
    }
    insert_entry(&mut payload_entries, SBOM_PATH, request.sbom_cyclonedx_json)?;
    insert_entry(&mut payload_entries, PROVENANCE_PATH, request.provenance_json)?;

    manifest.integrity.files = payload_entries
        .iter()
        .map(|(path, bytes)| SkillIntegrityEntry {
            path: path.clone(),
            sha256: sha256_hex(bytes.as_slice()),
        })
        .collect();

    let manifest_toml = toml::to_string_pretty(&manifest).map_err(|error| {
        SkillPackagingError::Serialization(format!("failed to serialize manifest: {error}"))
    })?;
    insert_entry(&mut payload_entries, SKILL_MANIFEST_PATH, manifest_toml.into_bytes())?;

    let payload_sha256 = compute_payload_hash_hex(
        payload_entries.iter().filter(|(path, _)| path.as_str() != SIGNATURE_PATH),
    );
    let signing_key = SigningKey::from_bytes(&request.signing_key);
    let verifying_key = VerifyingKey::from(&signing_key);
    let signature = signing_key.sign(payload_sha256.as_bytes());
    let signature_payload = SkillArtifactSignature {
        algorithm: SIGNATURE_ALGORITHM.to_owned(),
        publisher: manifest.publisher.clone(),
        key_id: key_id_for(&verifying_key),
        public_key_base64: BASE64_STANDARD.encode(verifying_key.as_bytes()),
        payload_sha256: payload_sha256.clone(),
        signature_base64: BASE64_STANDARD.encode(signature.to_bytes()),
        signed_at_unix_ms: now_unix_ms(),
    };
    let signature_json = serde_json::to_vec_pretty(&signature_payload).map_err(|error| {
        SkillPackagingError::Serialization(format!("failed to serialize signature: {error}"))
    })?;
    insert_entry(&mut payload_entries, SIGNATURE_PATH, signature_json)?;

    if payload_entries.len() > MAX_ENTRIES {
        return Err(SkillPackagingError::ArtifactTooManyEntries {
            actual: payload_entries.len(),
            limit: MAX_ENTRIES,
        });
    }
    let total_uncompressed = payload_entries.values().try_fold(0_usize, |sum, payload| {
        sum.checked_add(payload.len()).ok_or(SkillPackagingError::ArtifactTooLarge {
            actual: usize::MAX,
            limit: MAX_ARTIFACT_BYTES,
        })
    })?;
    if total_uncompressed > MAX_ARTIFACT_BYTES {
        return Err(SkillPackagingError::ArtifactTooLarge {
            actual: total_uncompressed,
            limit: MAX_ARTIFACT_BYTES,
        });
    }

    let artifact_bytes = encode_zip(payload_entries.iter())?;
    if artifact_bytes.len() > MAX_ARTIFACT_BYTES {
        return Err(SkillPackagingError::ArtifactTooLarge {
            actual: artifact_bytes.len(),
            limit: MAX_ARTIFACT_BYTES,
        });
    }
    Ok(SkillArtifactBuildOutput {
        artifact_bytes,
        manifest,
        payload_sha256,
        signature: signature_payload,
    })
}

pub(crate) fn parse_and_verify_artifact(
    entries: &BTreeMap<String, Vec<u8>>,
) -> Result<ParsedArtifact, SkillPackagingError> {
    let manifest_bytes = entries
        .get(SKILL_MANIFEST_PATH)
        .ok_or_else(|| SkillPackagingError::MissingArtifactEntry(SKILL_MANIFEST_PATH.to_owned()))?;
    let manifest_text = std::str::from_utf8(manifest_bytes.as_slice()).map_err(|error| {
        SkillPackagingError::ManifestParse(format!("manifest utf8 error: {error}"))
    })?;
    let manifest = parse_manifest_toml(manifest_text)?;

    let sbom = entries
        .get(SBOM_PATH)
        .ok_or_else(|| SkillPackagingError::MissingArtifactEntry(SBOM_PATH.to_owned()))?;
    validate_sbom_payload(sbom.as_slice())?;

    let provenance = entries
        .get(PROVENANCE_PATH)
        .ok_or_else(|| SkillPackagingError::MissingArtifactEntry(PROVENANCE_PATH.to_owned()))?;
    validate_provenance_payload(provenance.as_slice())?;

    if !entries.keys().any(|path| path.starts_with("modules/") && path.ends_with(".wasm")) {
        return Err(SkillPackagingError::MissingArtifactEntry("modules/*.wasm".to_owned()));
    }

    let signature_bytes = entries
        .get(SIGNATURE_PATH)
        .ok_or_else(|| SkillPackagingError::MissingArtifactEntry(SIGNATURE_PATH.to_owned()))?;
    let signature = serde_json::from_slice::<SkillArtifactSignature>(signature_bytes.as_slice())
        .map_err(|error| {
            SkillPackagingError::Serialization(format!("invalid signature: {error}"))
        })?;

    let payload_sha256 = compute_payload_hash_hex(
        entries.iter().filter(|(path, _)| path.as_str() != SIGNATURE_PATH),
    );
    if payload_sha256 != signature.payload_sha256 {
        return Err(SkillPackagingError::PayloadHashMismatch);
    }

    verify_signature(&signature, payload_sha256.as_str())?;
    if signature.publisher != manifest.publisher {
        return Err(SkillPackagingError::SignatureVerificationFailed);
    }

    let expected_integrity = entries
        .iter()
        .filter(|(path, _)| !matches!(path.as_str(), SKILL_MANIFEST_PATH | SIGNATURE_PATH))
        .map(|(path, bytes)| (path.clone(), sha256_hex(bytes.as_slice())))
        .collect::<BTreeMap<_, _>>();
    let declared_integrity = manifest
        .integrity
        .files
        .iter()
        .map(|entry| Ok((normalize_artifact_path(entry.path.as_str())?, entry.sha256.clone())))
        .collect::<Result<BTreeMap<_, _>, SkillPackagingError>>()?;
    if expected_integrity != declared_integrity {
        return Err(SkillPackagingError::ManifestValidation(
            "manifest integrity does not match artifact payload".to_owned(),
        ));
    }

    Ok(ParsedArtifact { manifest, signature, payload_sha256 })
}

fn verify_signature(
    payload: &SkillArtifactSignature,
    payload_sha256: &str,
) -> Result<(), SkillPackagingError> {
    let verifying_key = parse_verifying_key(payload)?;
    let signature_bytes = BASE64_STANDARD
        .decode(payload.signature_base64.as_bytes())
        .map_err(|_| SkillPackagingError::SignatureVerificationFailed)?;
    let signature_array: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| SkillPackagingError::SignatureVerificationFailed)?;
    let signature = Signature::from_bytes(&signature_array);
    verifying_key
        .verify(payload_sha256.as_bytes(), &signature)
        .map_err(|_| SkillPackagingError::SignatureVerificationFailed)
}

pub(crate) fn parse_verifying_key(
    payload: &SkillArtifactSignature,
) -> Result<VerifyingKey, SkillPackagingError> {
    if payload.algorithm != SIGNATURE_ALGORITHM {
        return Err(SkillPackagingError::SignatureVerificationFailed);
    }
    let bytes = BASE64_STANDARD
        .decode(payload.public_key_base64.as_bytes())
        .map_err(|_| SkillPackagingError::SignatureVerificationFailed)?;
    let array: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| SkillPackagingError::SignatureVerificationFailed)?;
    let key = VerifyingKey::from_bytes(&array)
        .map_err(|_| SkillPackagingError::SignatureVerificationFailed)?;
    if payload.key_id != key_id_for(&key) {
        return Err(SkillPackagingError::SignatureVerificationFailed);
    }
    Ok(key)
}

pub(crate) fn decode_zip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, SkillPackagingError> {
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err(SkillPackagingError::ArtifactTooLarge {
            actual: bytes.len(),
            limit: MAX_ARTIFACT_BYTES,
        });
    }
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|error| SkillPackagingError::Zip(error.to_string()))?;
    if archive.len() > MAX_ENTRIES {
        return Err(SkillPackagingError::ArtifactTooManyEntries {
            actual: archive.len(),
            limit: MAX_ENTRIES,
        });
    }
    let mut entries = BTreeMap::new();
    let mut total_uncompressed = 0_usize;
    for index in 0..archive.len() {
        let file =
            archive.by_index(index).map_err(|error| SkillPackagingError::Zip(error.to_string()))?;
        if file.is_dir() {
            continue;
        }
        let path = normalize_artifact_path(file.name())?;
        let declared_size = usize::try_from(file.size()).unwrap_or(usize::MAX);
        if declared_size > MAX_ENTRY_BYTES {
            return Err(SkillPackagingError::ArtifactEntryTooLarge {
                path,
                actual: declared_size,
                limit: MAX_ENTRY_BYTES,
            });
        }
        if total_uncompressed >= MAX_ARTIFACT_BYTES {
            return Err(SkillPackagingError::ArtifactTooLarge {
                actual: total_uncompressed,
                limit: MAX_ARTIFACT_BYTES,
            });
        }
        let remaining_total = MAX_ARTIFACT_BYTES - total_uncompressed;
        if declared_size > remaining_total {
            return Err(SkillPackagingError::ArtifactTooLarge {
                actual: total_uncompressed.saturating_add(declared_size),
                limit: MAX_ARTIFACT_BYTES,
            });
        }
        let entry_limit = remaining_total.min(MAX_ENTRY_BYTES);
        let mut payload = Vec::with_capacity(declared_size.min(entry_limit));
        let read_limit = u64::try_from(entry_limit).unwrap_or(u64::MAX).saturating_add(1);
        let mut limited_reader = file.take(read_limit);
        limited_reader
            .read_to_end(&mut payload)
            .map_err(|error| SkillPackagingError::Io(format!("zip read failed: {error}")))?;
        if payload.len() > entry_limit {
            if entry_limit < MAX_ENTRY_BYTES {
                return Err(SkillPackagingError::ArtifactTooLarge {
                    actual: total_uncompressed.saturating_add(payload.len()),
                    limit: MAX_ARTIFACT_BYTES,
                });
            }
            return Err(SkillPackagingError::ArtifactEntryTooLarge {
                path,
                actual: payload.len(),
                limit: MAX_ENTRY_BYTES,
            });
        }
        total_uncompressed = total_uncompressed.checked_add(payload.len()).ok_or(
            SkillPackagingError::ArtifactTooLarge { actual: usize::MAX, limit: MAX_ARTIFACT_BYTES },
        )?;
        if total_uncompressed > MAX_ARTIFACT_BYTES {
            return Err(SkillPackagingError::ArtifactTooLarge {
                actual: total_uncompressed,
                limit: MAX_ARTIFACT_BYTES,
            });
        }
        insert_entry(&mut entries, path.as_str(), payload)?;
    }
    Ok(entries)
}

pub(crate) fn encode_zip<'a>(
    entries: impl Iterator<Item = (&'a String, &'a Vec<u8>)>,
) -> Result<Vec<u8>, SkillPackagingError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    let mut writer = ZipWriter::new(Cursor::new(Vec::<u8>::new()));
    for (path, payload) in entries {
        writer
            .start_file(path, options)
            .map_err(|error| SkillPackagingError::Zip(error.to_string()))?;
        writer
            .write_all(payload.as_slice())
            .map_err(|error| SkillPackagingError::Io(format!("zip write failed: {error}")))?;
    }
    writer
        .finish()
        .map_err(|error| SkillPackagingError::Zip(error.to_string()))
        .map(|cursor| cursor.into_inner())
}

fn insert_entry(
    entries: &mut BTreeMap<String, Vec<u8>>,
    path: &str,
    payload: Vec<u8>,
) -> Result<(), SkillPackagingError> {
    if payload.len() > MAX_ENTRY_BYTES {
        return Err(SkillPackagingError::ArtifactEntryTooLarge {
            path: path.to_owned(),
            actual: payload.len(),
            limit: MAX_ENTRY_BYTES,
        });
    }
    let normalized = normalize_artifact_path(path)?;
    if entries.insert(normalized.clone(), payload).is_some() {
        return Err(SkillPackagingError::DuplicateArtifactEntry(normalized));
    }
    Ok(())
}

fn compute_payload_hash_hex<'a>(
    entries: impl Iterator<Item = (&'a String, &'a Vec<u8>)>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PAYLOAD_CONTEXT);
    for (path, payload) in entries {
        hash_len_prefixed(&mut hasher, path.as_bytes());
        hash_len_prefixed(&mut hasher, payload.as_slice());
    }
    hex::encode(hasher.finalize())
}

fn hash_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

pub(crate) fn normalize_artifact_path(raw: &str) -> Result<String, SkillPackagingError> {
    let normalized = raw.trim().replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') || normalized.contains('\0') {
        return Err(SkillPackagingError::InvalidArtifactPath(raw.to_owned()));
    }
    if normalized.contains(':') {
        return Err(SkillPackagingError::InvalidArtifactPath(raw.to_owned()));
    }
    let segments = normalized.split('/').collect::<Vec<_>>();
    if segments.is_empty()
        || segments.iter().any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
    {
        return Err(SkillPackagingError::InvalidArtifactPath(raw.to_owned()));
    }
    Ok(segments.join("/"))
}

fn key_id_for(key: &VerifyingKey) -> String {
    let digest = Sha256::digest(key.as_bytes());
    format!("ed25519:{}", hex::encode(&digest[..8]))
}

fn sha256_hex(payload: &[u8]) -> String {
    hex::encode(Sha256::digest(payload))
}

pub(crate) fn now_unix_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}
