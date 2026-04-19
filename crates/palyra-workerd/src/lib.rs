use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;

const MAX_WORKER_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerAttestation {
    pub worker_id: String,
    pub image_digest_sha256: String,
    pub build_digest_sha256: String,
    pub artifact_digest_sha256: String,
    pub egress_proxy_attested: bool,
    pub issued_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerAttestationExpectation {
    pub require_egress_proxy: bool,
    pub image_digest_sha256: Option<String>,
    pub build_digest_sha256: Option<String>,
    pub artifact_digest_sha256: Option<String>,
}

impl Default for WorkerAttestationExpectation {
    fn default() -> Self {
        Self {
            require_egress_proxy: true,
            image_digest_sha256: None,
            build_digest_sha256: None,
            artifact_digest_sha256: None,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkerAttestationError {
    #[error("worker attestation missing worker identifier")]
    MissingWorkerId,
    #[error("worker attestation is expired")]
    Expired,
    #[error("worker attestation is not yet valid")]
    NotYetValid,
    #[error("worker attestation does not include an attested egress proxy binding")]
    MissingEgressProxyBinding,
    #[error("worker attestation {field} digest did not match the expected value")]
    DigestMismatch { field: &'static str },
}

impl WorkerAttestation {
    pub fn validate(
        &self,
        expected: &WorkerAttestationExpectation,
        now_unix_ms: i64,
    ) -> Result<(), WorkerAttestationError> {
        if self.worker_id.trim().is_empty() || self.worker_id.len() > MAX_WORKER_ID_BYTES {
            return Err(WorkerAttestationError::MissingWorkerId);
        }
        if self.issued_at_unix_ms > now_unix_ms {
            return Err(WorkerAttestationError::NotYetValid);
        }
        if self.expires_at_unix_ms <= now_unix_ms {
            return Err(WorkerAttestationError::Expired);
        }
        if expected.require_egress_proxy && !self.egress_proxy_attested {
            return Err(WorkerAttestationError::MissingEgressProxyBinding);
        }
        if expected
            .image_digest_sha256
            .as_deref()
            .is_some_and(|expected_digest| expected_digest != self.image_digest_sha256)
        {
            return Err(WorkerAttestationError::DigestMismatch { field: "image" });
        }
        if expected
            .build_digest_sha256
            .as_deref()
            .is_some_and(|expected_digest| expected_digest != self.build_digest_sha256)
        {
            return Err(WorkerAttestationError::DigestMismatch { field: "build" });
        }
        if expected
            .artifact_digest_sha256
            .as_deref()
            .is_some_and(|expected_digest| expected_digest != self.artifact_digest_sha256)
        {
            return Err(WorkerAttestationError::DigestMismatch { field: "artifact" });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerWorkspaceScope {
    pub workspace_root: String,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerArtifactTransport {
    pub input_manifest_sha256: String,
    pub output_manifest_sha256: String,
    pub log_stream_id: String,
    pub scratch_directory_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLeaseRequest {
    pub run_id: String,
    pub ttl_ms: u64,
    pub workspace_scope: WorkerWorkspaceScope,
    pub artifact_transport: WorkerArtifactTransport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLease {
    pub lease_id: String,
    pub worker_id: String,
    pub run_id: String,
    pub expires_at_unix_ms: i64,
    pub workspace_scope: WorkerWorkspaceScope,
    pub artifact_transport: WorkerArtifactTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLifecycleState {
    Registered,
    Assigned,
    Completed,
    Failed,
    Orphaned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerCleanupReport {
    pub removed_workspace_scope: bool,
    pub removed_artifacts: bool,
    pub removed_logs: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLifecycleEvent {
    pub worker_id: String,
    pub state: WorkerLifecycleState,
    pub run_id: Option<String>,
    pub reason_code: String,
    pub timestamp_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorkerFleetSnapshot {
    pub registered_workers: usize,
    pub attested_workers: usize,
    pub active_leases: usize,
    pub orphaned_workers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerFleetPolicy {
    pub max_ttl_ms: u64,
    pub attestation: WorkerAttestationExpectation,
}

impl Default for WorkerFleetPolicy {
    fn default() -> Self {
        Self { max_ttl_ms: 15 * 60 * 1_000, attestation: WorkerAttestationExpectation::default() }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkerLifecycleError {
    #[error(transparent)]
    Attestation(#[from] WorkerAttestationError),
    #[error("worker '{0}' is already registered")]
    AlreadyRegistered(String),
    #[error("worker '{0}' is not registered")]
    UnknownWorker(String),
    #[error("requested worker lease ttl exceeds the configured maximum")]
    TtlExceeded,
    #[error("worker '{0}' already has an active lease")]
    LeaseAlreadyActive(String),
    #[error("worker cleanup failed and the worker stayed fail-closed")]
    CleanupFailed,
}

#[derive(Debug, Clone)]
struct WorkerRecord {
    attestation: WorkerAttestation,
    state: WorkerLifecycleState,
    lease: Option<WorkerLease>,
}

#[derive(Debug, Default)]
pub struct WorkerFleetManager {
    workers: BTreeMap<String, WorkerRecord>,
}

impl WorkerFleetManager {
    #[must_use]
    pub fn snapshot(&self) -> WorkerFleetSnapshot {
        let registered_workers = self.workers.len();
        let attested_workers =
            self.workers.values().filter(|worker| worker.attestation.egress_proxy_attested).count();
        let active_leases = self.workers.values().filter(|worker| worker.lease.is_some()).count();
        let orphaned_workers = self
            .workers
            .values()
            .filter(|worker| matches!(worker.state, WorkerLifecycleState::Orphaned))
            .count();
        WorkerFleetSnapshot {
            registered_workers,
            attested_workers,
            active_leases,
            orphaned_workers,
        }
    }

    pub fn register_worker(
        &mut self,
        attestation: WorkerAttestation,
        policy: &WorkerFleetPolicy,
        now_unix_ms: i64,
    ) -> Result<WorkerLifecycleEvent, WorkerLifecycleError> {
        attestation.validate(&policy.attestation, now_unix_ms)?;
        if self.workers.contains_key(attestation.worker_id.as_str()) {
            return Err(WorkerLifecycleError::AlreadyRegistered(attestation.worker_id));
        }
        let worker_id = attestation.worker_id.clone();
        self.workers.insert(
            worker_id.clone(),
            WorkerRecord { attestation, state: WorkerLifecycleState::Registered, lease: None },
        );
        Ok(WorkerLifecycleEvent {
            worker_id,
            state: WorkerLifecycleState::Registered,
            run_id: None,
            reason_code: "worker.registered".to_owned(),
            timestamp_unix_ms: now_unix_ms,
        })
    }

    pub fn assign_work(
        &mut self,
        worker_id: &str,
        request: WorkerLeaseRequest,
        policy: &WorkerFleetPolicy,
        now_unix_ms: i64,
    ) -> Result<(WorkerLease, WorkerLifecycleEvent), WorkerLifecycleError> {
        let worker = self
            .workers
            .get_mut(worker_id)
            .ok_or_else(|| WorkerLifecycleError::UnknownWorker(worker_id.to_owned()))?;
        worker.attestation.validate(&policy.attestation, now_unix_ms)?;
        if request.ttl_ms > policy.max_ttl_ms {
            return Err(WorkerLifecycleError::TtlExceeded);
        }
        if worker.lease.is_some() {
            return Err(WorkerLifecycleError::LeaseAlreadyActive(worker_id.to_owned()));
        }
        let lease = WorkerLease {
            lease_id: Ulid::new().to_string(),
            worker_id: worker_id.to_owned(),
            run_id: request.run_id.clone(),
            expires_at_unix_ms: now_unix_ms.saturating_add(request.ttl_ms as i64),
            workspace_scope: request.workspace_scope,
            artifact_transport: request.artifact_transport,
        };
        worker.state = WorkerLifecycleState::Assigned;
        worker.lease = Some(lease.clone());
        Ok((
            lease.clone(),
            WorkerLifecycleEvent {
                worker_id: worker_id.to_owned(),
                state: WorkerLifecycleState::Assigned,
                run_id: Some(lease.run_id.clone()),
                reason_code: "worker.assigned".to_owned(),
                timestamp_unix_ms: now_unix_ms,
            },
        ))
    }

    pub fn complete_work(
        &mut self,
        worker_id: &str,
        cleanup: &WorkerCleanupReport,
        now_unix_ms: i64,
    ) -> Result<WorkerLifecycleEvent, WorkerLifecycleError> {
        let worker = self
            .workers
            .get_mut(worker_id)
            .ok_or_else(|| WorkerLifecycleError::UnknownWorker(worker_id.to_owned()))?;
        let run_id = worker.lease.as_ref().map(|lease| lease.run_id.clone());
        if cleanup.failure_reason.is_some()
            || !cleanup.removed_workspace_scope
            || !cleanup.removed_artifacts
            || !cleanup.removed_logs
        {
            worker.state = WorkerLifecycleState::Failed;
            worker.lease = None;
            return Err(WorkerLifecycleError::CleanupFailed);
        }
        worker.state = WorkerLifecycleState::Completed;
        worker.lease = None;
        Ok(WorkerLifecycleEvent {
            worker_id: worker_id.to_owned(),
            state: WorkerLifecycleState::Completed,
            run_id,
            reason_code: "worker.completed".to_owned(),
            timestamp_unix_ms: now_unix_ms,
        })
    }

    pub fn reap_expired_workers(&mut self, now_unix_ms: i64) -> Vec<WorkerLifecycleEvent> {
        let mut events = Vec::new();
        for (worker_id, worker) in &mut self.workers {
            let expired =
                worker.lease.as_ref().is_some_and(|lease| lease.expires_at_unix_ms <= now_unix_ms);
            if expired {
                let run_id = worker.lease.as_ref().map(|lease| lease.run_id.clone());
                worker.state = WorkerLifecycleState::Orphaned;
                worker.lease = None;
                events.push(WorkerLifecycleEvent {
                    worker_id: worker_id.clone(),
                    state: WorkerLifecycleState::Orphaned,
                    run_id,
                    reason_code: "worker.ttl_expired".to_owned(),
                    timestamp_unix_ms: now_unix_ms,
                });
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WorkerArtifactTransport, WorkerAttestation, WorkerCleanupReport, WorkerFleetManager,
        WorkerFleetPolicy, WorkerLeaseRequest, WorkerLifecycleError, WorkerLifecycleState,
        WorkerWorkspaceScope,
    };

    fn attestation(worker_id: &str) -> WorkerAttestation {
        WorkerAttestation {
            worker_id: worker_id.to_owned(),
            image_digest_sha256: "img".repeat(16),
            build_digest_sha256: "bld".repeat(16),
            artifact_digest_sha256: "art".repeat(16),
            egress_proxy_attested: true,
            issued_at_unix_ms: 1_000,
            expires_at_unix_ms: 10_000,
        }
    }

    fn lease_request(run_id: &str, ttl_ms: u64) -> WorkerLeaseRequest {
        WorkerLeaseRequest {
            run_id: run_id.to_owned(),
            ttl_ms,
            workspace_scope: WorkerWorkspaceScope {
                workspace_root: "/workspace".to_owned(),
                allowed_paths: vec!["src".to_owned()],
                read_only: false,
            },
            artifact_transport: WorkerArtifactTransport {
                input_manifest_sha256: "in".repeat(32),
                output_manifest_sha256: "out".repeat(32),
                log_stream_id: "log-stream".to_owned(),
                scratch_directory_id: "scratch".to_owned(),
            },
        }
    }

    #[test]
    fn worker_lifecycle_supports_successful_handshake_assignment_and_cleanup() {
        let mut manager = WorkerFleetManager::default();
        let policy = WorkerFleetPolicy::default();

        let register = manager
            .register_worker(attestation("worker-a"), &policy, 2_000)
            .expect("worker should register");
        assert_eq!(register.reason_code, "worker.registered");

        let (lease, assign) = manager
            .assign_work("worker-a", lease_request("run-1", 500), &policy, 2_500)
            .expect("worker should accept a lease");
        assert_eq!(lease.run_id, "run-1");
        assert_eq!(assign.state, WorkerLifecycleState::Assigned);

        let complete = manager
            .complete_work(
                "worker-a",
                &WorkerCleanupReport {
                    removed_workspace_scope: true,
                    removed_artifacts: true,
                    removed_logs: true,
                    failure_reason: None,
                },
                3_000,
            )
            .expect("cleanup should succeed");
        assert_eq!(complete.state, WorkerLifecycleState::Completed);
        assert_eq!(manager.snapshot().active_leases, 0);
    }

    #[test]
    fn worker_registration_rejects_missing_egress_proxy_attestation() {
        let mut manager = WorkerFleetManager::default();
        let policy = WorkerFleetPolicy::default();
        let mut worker_attestation = attestation("worker-b");
        worker_attestation.egress_proxy_attested = false;

        let error = manager
            .register_worker(worker_attestation, &policy, 2_000)
            .expect_err("egress proxy binding should be required");
        assert!(matches!(
            error,
            WorkerLifecycleError::Attestation(
                super::WorkerAttestationError::MissingEgressProxyBinding
            )
        ));
    }

    #[test]
    fn worker_cleanup_failure_stays_fail_closed() {
        let mut manager = WorkerFleetManager::default();
        let policy = WorkerFleetPolicy::default();
        manager.register_worker(attestation("worker-c"), &policy, 2_000).unwrap();
        manager.assign_work("worker-c", lease_request("run-2", 500), &policy, 2_500).unwrap();

        let error = manager
            .complete_work(
                "worker-c",
                &WorkerCleanupReport {
                    removed_workspace_scope: false,
                    removed_artifacts: true,
                    removed_logs: true,
                    failure_reason: Some("artifact cleanup failure".to_owned()),
                },
                3_000,
            )
            .expect_err("cleanup failure should not be ignored");
        assert_eq!(error, WorkerLifecycleError::CleanupFailed);
    }

    #[test]
    fn worker_ttl_reap_marks_orphaned_instances() {
        let mut manager = WorkerFleetManager::default();
        let policy = WorkerFleetPolicy::default();
        manager.register_worker(attestation("worker-d"), &policy, 2_000).unwrap();
        manager.assign_work("worker-d", lease_request("run-3", 250), &policy, 2_500).unwrap();

        let events = manager.reap_expired_workers(2_751);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state, WorkerLifecycleState::Orphaned);
        assert_eq!(manager.snapshot().orphaned_workers, 1);
    }
}
