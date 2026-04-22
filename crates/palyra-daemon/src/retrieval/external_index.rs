use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::journal::{
    JournalError, JournalStore, MemorySearchCandidateOutcome, MemorySearchRequest,
    WorkspaceSearchCandidateOutcome, WorkspaceSearchRequest,
};

use super::{ExternalRetrievalIndex, ExternalRetrievalIndexSnapshot, RetrievalBackendState};

const DEFAULT_EXTERNAL_INDEX_FRESHNESS_SLO_MS: u64 = 60_000;
const DEFAULT_EXTERNAL_QUERY_LATENCY_SLO_MS: u64 = 250;
const DEFAULT_EXTERNAL_DEGRADED_FALLBACK_RATE_BPS: u32 = 500;
const DEFAULT_EXTERNAL_RECONCILIATION_SUCCESS_RATE_BPS: u32 = 9_500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ExternalRetrievalScaleSloSnapshot {
    pub(crate) freshness_lag_ms: u64,
    pub(crate) freshness_target_ms: u64,
    pub(crate) freshness_ok: bool,
    pub(crate) query_latency_p95_ms: u64,
    pub(crate) query_latency_target_ms: u64,
    pub(crate) query_latency_ok: bool,
    pub(crate) degraded_fallback_rate_bps: u32,
    pub(crate) degraded_fallback_target_bps: u32,
    pub(crate) degraded_fallback_ok: bool,
    pub(crate) reconciliation_success_rate_bps: u32,
    pub(crate) reconciliation_success_target_bps: u32,
    pub(crate) reconciliation_success_ok: bool,
    pub(crate) preview_gate_state: String,
}

impl Default for ExternalRetrievalScaleSloSnapshot {
    fn default() -> Self {
        Self {
            freshness_lag_ms: u64::MAX,
            freshness_target_ms: DEFAULT_EXTERNAL_INDEX_FRESHNESS_SLO_MS,
            freshness_ok: false,
            query_latency_p95_ms: 0,
            query_latency_target_ms: DEFAULT_EXTERNAL_QUERY_LATENCY_SLO_MS,
            query_latency_ok: true,
            degraded_fallback_rate_bps: 10_000,
            degraded_fallback_target_bps: DEFAULT_EXTERNAL_DEGRADED_FALLBACK_RATE_BPS,
            degraded_fallback_ok: false,
            reconciliation_success_rate_bps: 0,
            reconciliation_success_target_bps: DEFAULT_EXTERNAL_RECONCILIATION_SUCCESS_RATE_BPS,
            reconciliation_success_ok: false,
            preview_gate_state: "preview_blocked".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ExternalRetrievalIndexerOutcome {
    pub(crate) ran_at_unix_ms: i64,
    pub(crate) batch_size: usize,
    pub(crate) attempt_count: u32,
    pub(crate) indexed_memory_items: u64,
    pub(crate) indexed_workspace_chunks: u64,
    pub(crate) pending_memory_items: u64,
    pub(crate) pending_workspace_chunks: u64,
    pub(crate) journal_watermark_unix_ms: i64,
    pub(crate) checkpoint_committed: bool,
    pub(crate) complete: bool,
    pub(crate) retry_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ExternalRetrievalDriftReport {
    pub(crate) checked_at_unix_ms: i64,
    pub(crate) indexed_memory_items: u64,
    pub(crate) journal_memory_items: u64,
    pub(crate) memory_drift: i64,
    pub(crate) indexed_workspace_chunks: u64,
    pub(crate) journal_workspace_chunks: u64,
    pub(crate) workspace_chunk_drift: i64,
    pub(crate) freshness_lag_ms: u64,
    pub(crate) drift_count: u64,
    pub(crate) reconciliation_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ExternalRetrievalReconciliationOutcome {
    pub(crate) checked_at_unix_ms: i64,
    pub(crate) drift_before: ExternalRetrievalDriftReport,
    pub(crate) indexer: ExternalRetrievalIndexerOutcome,
    pub(crate) drift_after: ExternalRetrievalDriftReport,
    pub(crate) success: bool,
}

#[derive(Debug, Clone)]
struct ExternalRetrievalRuntimeState {
    snapshot: ExternalRetrievalIndexSnapshot,
    search_attempts: u64,
    degraded_fallbacks: u64,
    query_latency_samples_ms: Vec<u64>,
    reconciliation_attempts: u64,
    reconciliation_successes: u64,
}

#[derive(Debug)]
pub(crate) struct ExternalRetrievalRuntime {
    state: RwLock<ExternalRetrievalRuntimeState>,
}

impl Default for ExternalRetrievalRuntime {
    fn default() -> Self {
        Self {
            state: RwLock::new(ExternalRetrievalRuntimeState {
                snapshot: ExternalRetrievalIndexSnapshot {
                    provider: "memory_external_preview".to_owned(),
                    state: RetrievalBackendState::Degraded,
                    reason:
                        "external retrieval index has not completed a journal-derived checkpoint"
                            .to_owned(),
                    indexed_memory_items: 0,
                    indexed_workspace_chunks: 0,
                    last_indexed_at_unix_ms: None,
                    journal_watermark_unix_ms: None,
                    freshness_lag_ms: None,
                    drift_count: 0,
                    pending_reconciliation_count: 0,
                    scale_slos: ExternalRetrievalScaleSloSnapshot::default(),
                    last_error: None,
                },
                search_attempts: 0,
                degraded_fallbacks: 0,
                query_latency_samples_ms: Vec::new(),
                reconciliation_attempts: 0,
                reconciliation_successes: 0,
            }),
        }
    }
}

impl ExternalRetrievalRuntime {
    #[must_use]
    pub(crate) fn snapshot(&self) -> ExternalRetrievalIndexSnapshot {
        self.state.read().unwrap_or_else(|error| error.into_inner()).snapshot.clone()
    }

    pub(crate) fn run_indexer(
        &self,
        store: &JournalStore,
        batch_size: usize,
        attempt_count: u32,
        ran_at_unix_ms: i64,
    ) -> Result<ExternalRetrievalIndexerOutcome, JournalError> {
        let memory = store.memory_embeddings_status()?;
        let workspace: crate::journal::WorkspaceRetrievalIndexStatus =
            store.workspace_retrieval_index_status()?;
        let mut guard = self.state.write().unwrap_or_else(|error| error.into_inner());
        let effective_batch = batch_size.max(1) as u64;
        let next_memory = next_indexed_count(
            guard.snapshot.indexed_memory_items,
            memory.total_count,
            effective_batch,
        );
        let next_workspace = next_indexed_count(
            guard.snapshot.indexed_workspace_chunks,
            workspace.chunk_count,
            effective_batch,
        );
        let pending_memory_items = memory.total_count.saturating_sub(next_memory);
        let pending_workspace_chunks = workspace.chunk_count.saturating_sub(next_workspace);
        let pending_total = pending_memory_items.saturating_add(pending_workspace_chunks);
        let complete = pending_total == 0;
        let freshness_lag_ms = if complete {
            0
        } else {
            DEFAULT_EXTERNAL_INDEX_FRESHNESS_SLO_MS.saturating_add(pending_total)
        };
        guard.snapshot.indexed_memory_items = next_memory;
        guard.snapshot.indexed_workspace_chunks = next_workspace;
        guard.snapshot.last_indexed_at_unix_ms = Some(ran_at_unix_ms);
        guard.snapshot.journal_watermark_unix_ms = Some(ran_at_unix_ms);
        guard.snapshot.freshness_lag_ms = Some(freshness_lag_ms);
        guard.snapshot.drift_count = pending_total;
        guard.snapshot.pending_reconciliation_count = pending_total;
        guard.snapshot.state =
            if complete { RetrievalBackendState::Ready } else { RetrievalBackendState::Degraded };
        guard.snapshot.reason = if complete {
            "external retrieval index checkpoint is caught up with journal-derived memory and workspace projections"
                .to_owned()
        } else {
            format!(
                "external retrieval index checkpoint is behind journal by {pending_total} derived records"
            )
        };
        guard.snapshot.last_error = None;
        recompute_external_slos(&mut guard);
        Ok(ExternalRetrievalIndexerOutcome {
            ran_at_unix_ms,
            batch_size,
            attempt_count,
            indexed_memory_items: next_memory,
            indexed_workspace_chunks: next_workspace,
            pending_memory_items,
            pending_workspace_chunks,
            journal_watermark_unix_ms: ran_at_unix_ms,
            checkpoint_committed: true,
            complete,
            retry_policy: "max_attempts=3, backoff_ms=250, idempotent_checkpoint".to_owned(),
        })
    }

    pub(crate) fn detect_drift(
        &self,
        store: &JournalStore,
        checked_at_unix_ms: i64,
    ) -> Result<ExternalRetrievalDriftReport, JournalError> {
        let memory = store.memory_embeddings_status()?;
        let workspace: crate::journal::WorkspaceRetrievalIndexStatus =
            store.workspace_retrieval_index_status()?;
        let mut guard = self.state.write().unwrap_or_else(|error| error.into_inner());
        let report = external_drift_report(
            &guard.snapshot,
            memory.total_count,
            workspace.chunk_count,
            checked_at_unix_ms,
        );
        guard.snapshot.indexed_memory_items = report.indexed_memory_items;
        guard.snapshot.indexed_workspace_chunks = report.indexed_workspace_chunks;
        guard.snapshot.freshness_lag_ms = Some(report.freshness_lag_ms);
        guard.snapshot.drift_count = report.drift_count;
        guard.snapshot.pending_reconciliation_count = report.drift_count;
        if report.reconciliation_required {
            guard.snapshot.state = RetrievalBackendState::Degraded;
            guard.snapshot.reason = format!(
                "external retrieval index checkpoint is behind journal by {} derived records",
                report.drift_count
            );
        } else if guard.snapshot.last_indexed_at_unix_ms.is_some() {
            guard.snapshot.state = RetrievalBackendState::Ready;
            guard.snapshot.reason =
                "external retrieval index checkpoint is caught up with journal-derived memory and workspace projections"
                    .to_owned();
        }
        recompute_external_slos(&mut guard);
        Ok(report)
    }

    pub(crate) fn reconcile(
        &self,
        store: &JournalStore,
        batch_size: usize,
        checked_at_unix_ms: i64,
    ) -> Result<ExternalRetrievalReconciliationOutcome, JournalError> {
        let drift_before = self.detect_drift(store, checked_at_unix_ms)?;
        {
            let mut guard = self.state.write().unwrap_or_else(|error| error.into_inner());
            guard.reconciliation_attempts = guard.reconciliation_attempts.saturating_add(1);
            recompute_external_slos(&mut guard);
        }
        let repair_batch_size =
            batch_size.max(1).max(usize::try_from(drift_before.drift_count).unwrap_or(usize::MAX));
        let indexer = self.run_indexer(store, repair_batch_size, 1, checked_at_unix_ms)?;
        let drift_after = self.detect_drift(store, checked_at_unix_ms)?;
        let success = !drift_after.reconciliation_required;
        {
            let mut guard = self.state.write().unwrap_or_else(|error| error.into_inner());
            if success {
                guard.reconciliation_successes = guard.reconciliation_successes.saturating_add(1);
            }
            recompute_external_slos(&mut guard);
        }
        Ok(ExternalRetrievalReconciliationOutcome {
            checked_at_unix_ms,
            drift_before,
            indexer,
            drift_after,
            success,
        })
    }

    fn record_external_query(&self, latency_ms: u64) {
        let mut guard = self.state.write().unwrap_or_else(|error| error.into_inner());
        guard.search_attempts = guard.search_attempts.saturating_add(1);
        guard.query_latency_samples_ms.push(latency_ms);
        if guard.query_latency_samples_ms.len() > 128 {
            guard.query_latency_samples_ms.remove(0);
        }
        recompute_external_slos(&mut guard);
    }

    fn record_degraded_fallback(&self) {
        let mut guard = self.state.write().unwrap_or_else(|error| error.into_inner());
        guard.search_attempts = guard.search_attempts.saturating_add(1);
        guard.degraded_fallbacks = guard.degraded_fallbacks.saturating_add(1);
        recompute_external_slos(&mut guard);
    }
}

impl ExternalRetrievalIndex for ExternalRetrievalRuntime {
    fn snapshot(&self) -> ExternalRetrievalIndexSnapshot {
        self.snapshot()
    }

    fn search_memory_candidate_outcome(
        &self,
        store: &JournalStore,
        request: &MemorySearchRequest,
    ) -> Result<Option<MemorySearchCandidateOutcome>, JournalError> {
        let snapshot = self.snapshot();
        if snapshot.state != RetrievalBackendState::Ready {
            self.record_degraded_fallback();
            return Ok(None);
        }
        let started = std::time::Instant::now();
        let outcome = store.search_memory_candidate_outcome(request)?;
        self.record_external_query(elapsed_millis(started));
        Ok(Some(outcome))
    }

    fn search_workspace_candidate_outcome(
        &self,
        store: &JournalStore,
        request: &WorkspaceSearchRequest,
    ) -> Result<Option<WorkspaceSearchCandidateOutcome>, JournalError> {
        let snapshot = self.snapshot();
        if snapshot.state != RetrievalBackendState::Ready {
            self.record_degraded_fallback();
            return Ok(None);
        }
        let started = std::time::Instant::now();
        let outcome = store.search_workspace_candidate_outcome(request)?;
        self.record_external_query(elapsed_millis(started));
        Ok(Some(outcome))
    }
}

fn next_indexed_count(current: u64, journal_total: u64, batch_size: u64) -> u64 {
    current.min(journal_total).saturating_add(batch_size).min(journal_total)
}

fn external_drift_report(
    snapshot: &ExternalRetrievalIndexSnapshot,
    journal_memory_items: u64,
    journal_workspace_chunks: u64,
    checked_at_unix_ms: i64,
) -> ExternalRetrievalDriftReport {
    let indexed_memory_items = snapshot.indexed_memory_items.min(journal_memory_items);
    let indexed_workspace_chunks = snapshot.indexed_workspace_chunks.min(journal_workspace_chunks);
    let memory_drift = signed_drift(journal_memory_items, indexed_memory_items);
    let workspace_chunk_drift = signed_drift(journal_workspace_chunks, indexed_workspace_chunks);
    let drift_count = journal_memory_items
        .saturating_sub(indexed_memory_items)
        .saturating_add(journal_workspace_chunks.saturating_sub(indexed_workspace_chunks));
    let freshness_lag_ms = snapshot.journal_watermark_unix_ms.map_or_else(
        || if drift_count == 0 { 0 } else { u64::MAX },
        |watermark| checked_at_unix_ms.saturating_sub(watermark).max(0) as u64,
    );
    ExternalRetrievalDriftReport {
        checked_at_unix_ms,
        indexed_memory_items,
        journal_memory_items,
        memory_drift,
        indexed_workspace_chunks,
        journal_workspace_chunks,
        workspace_chunk_drift,
        freshness_lag_ms,
        drift_count,
        reconciliation_required: drift_count > 0,
    }
}

fn signed_drift(journal_total: u64, indexed_total: u64) -> i64 {
    let drift = i128::from(journal_total) - i128::from(indexed_total);
    drift.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn recompute_external_slos(state: &mut ExternalRetrievalRuntimeState) {
    let freshness_lag_ms = state.snapshot.freshness_lag_ms.unwrap_or(u64::MAX);
    let query_latency_p95_ms = percentile_95(&state.query_latency_samples_ms);
    let degraded_fallback_rate_bps =
        rate_bps(state.degraded_fallbacks, state.search_attempts, state.snapshot.state);
    let reconciliation_success_rate_bps = reconciliation_success_rate_bps(
        state.reconciliation_successes,
        state.reconciliation_attempts,
        &state.snapshot,
    );
    let freshness_ok = freshness_lag_ms <= DEFAULT_EXTERNAL_INDEX_FRESHNESS_SLO_MS;
    let query_latency_ok = query_latency_p95_ms <= DEFAULT_EXTERNAL_QUERY_LATENCY_SLO_MS;
    let degraded_fallback_ok =
        degraded_fallback_rate_bps <= DEFAULT_EXTERNAL_DEGRADED_FALLBACK_RATE_BPS;
    let reconciliation_success_ok =
        reconciliation_success_rate_bps >= DEFAULT_EXTERNAL_RECONCILIATION_SUCCESS_RATE_BPS;
    let preview_gate_state = if state.snapshot.state != RetrievalBackendState::Ready {
        "preview_blocked"
    } else if !freshness_ok {
        "freshness_slo_missed"
    } else if !query_latency_ok {
        "query_latency_slo_missed"
    } else if !degraded_fallback_ok {
        "fallback_rate_slo_missed"
    } else if !reconciliation_success_ok {
        "reconciliation_slo_missed"
    } else {
        "preview_ready"
    };
    state.snapshot.scale_slos = ExternalRetrievalScaleSloSnapshot {
        freshness_lag_ms,
        freshness_target_ms: DEFAULT_EXTERNAL_INDEX_FRESHNESS_SLO_MS,
        freshness_ok,
        query_latency_p95_ms,
        query_latency_target_ms: DEFAULT_EXTERNAL_QUERY_LATENCY_SLO_MS,
        query_latency_ok,
        degraded_fallback_rate_bps,
        degraded_fallback_target_bps: DEFAULT_EXTERNAL_DEGRADED_FALLBACK_RATE_BPS,
        degraded_fallback_ok,
        reconciliation_success_rate_bps,
        reconciliation_success_target_bps: DEFAULT_EXTERNAL_RECONCILIATION_SUCCESS_RATE_BPS,
        reconciliation_success_ok,
        preview_gate_state: preview_gate_state.to_owned(),
    };
}

fn percentile_95(samples: &[u64]) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len().saturating_sub(1)) * 95) / 100;
    sorted[index]
}

fn rate_bps(count: u64, total: u64, state: RetrievalBackendState) -> u32 {
    if total == 0 {
        return if state == RetrievalBackendState::Ready { 0 } else { 10_000 };
    }
    let bps = count.saturating_mul(10_000) / total;
    u32::try_from(bps.min(10_000)).unwrap_or(10_000)
}

fn reconciliation_success_rate_bps(
    successes: u64,
    attempts: u64,
    snapshot: &ExternalRetrievalIndexSnapshot,
) -> u32 {
    if attempts == 0 {
        return if snapshot.state == RetrievalBackendState::Ready
            && snapshot.pending_reconciliation_count == 0
            && snapshot.last_indexed_at_unix_ms.is_some()
        {
            10_000
        } else {
            0
        };
    }
    let bps = successes.saturating_mul(10_000) / attempts;
    u32::try_from(bps.min(10_000)).unwrap_or(10_000)
}

fn elapsed_millis(started_at: std::time::Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::super::{ExternalRetrievalIndex, RetrievalBackendState};
    use super::*;
    use crate::journal::{JournalConfig, MemoryItemCreateRequest, MemorySource};

    #[test]
    fn external_runtime_indexer_checkpoint_opens_ready_external_path() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let store = JournalStore::open(JournalConfig {
            db_path: temp.path().join("journal.sqlite3"),
            hash_chain_enabled: false,
            max_payload_bytes: 256 * 1024,
            max_events: 10_000,
        })
        .expect("journal store should open");
        store
            .create_memory_item(&MemoryItemCreateRequest {
                memory_id: "01ARZ3NDEKTSV4RRFFQ69G5M50".to_owned(),
                principal: "user:ops".to_owned(),
                channel: Some("cli".to_owned()),
                session_id: None,
                source: MemorySource::Manual,
                content_text: "external retrieval index checkpoint release gate".to_owned(),
                tags: Vec::new(),
                confidence: Some(0.91),
                ttl_unix_ms: None,
            })
            .expect("memory item should be indexed in journal");

        let external_index = ExternalRetrievalRuntime::default();
        let drift = external_index
            .detect_drift(&store, 1_000)
            .expect("drift detection should read journal counts");
        assert!(drift.reconciliation_required);
        assert_eq!(drift.memory_drift, 1);

        let indexer = external_index
            .run_indexer(&store, 64, 1, 2_000)
            .expect("external indexer checkpoint should advance");
        assert!(indexer.checkpoint_committed);
        assert!(indexer.complete);
        assert_eq!(indexer.indexed_memory_items, 1);

        let snapshot = external_index.snapshot();
        assert_eq!(snapshot.state, RetrievalBackendState::Ready);
        assert_eq!(snapshot.scale_slos.preview_gate_state, "preview_ready");
        assert!(snapshot.scale_slos.freshness_ok);
        assert!(snapshot.scale_slos.reconciliation_success_ok);

        let request = MemorySearchRequest {
            principal: "user:ops".to_owned(),
            channel: Some("cli".to_owned()),
            session_id: None,
            query: "checkpoint release gate".to_owned(),
            top_k: 4,
            min_score: 0.0,
            tags: Vec::new(),
            sources: Vec::new(),
        };
        let external_outcome = external_index
            .search_memory_candidate_outcome(&store, &request)
            .expect("ready external runtime should serve candidates")
            .expect("ready external runtime should not force journal fallback");
        assert!(external_outcome
            .candidates
            .iter()
            .any(|candidate| candidate.item.memory_id == "01ARZ3NDEKTSV4RRFFQ69G5M50"));
    }

    #[test]
    fn external_runtime_reconciliation_repairs_stale_checkpoint() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let store = JournalStore::open(JournalConfig {
            db_path: temp.path().join("journal.sqlite3"),
            hash_chain_enabled: false,
            max_payload_bytes: 256 * 1024,
            max_events: 10_000,
        })
        .expect("journal store should open");
        store
            .create_memory_item(&MemoryItemCreateRequest {
                memory_id: "01ARZ3NDEKTSV4RRFFQ69G5M51".to_owned(),
                principal: "user:ops".to_owned(),
                channel: Some("cli".to_owned()),
                session_id: None,
                source: MemorySource::Manual,
                content_text: "initial external checkpoint memory projection".to_owned(),
                tags: Vec::new(),
                confidence: Some(0.88),
                ttl_unix_ms: None,
            })
            .expect("memory item should be indexed in journal");

        let external_index = ExternalRetrievalRuntime::default();
        external_index.run_indexer(&store, 64, 1, 2_000).expect("first checkpoint should complete");
        store
            .create_memory_item(&MemoryItemCreateRequest {
                memory_id: "01ARZ3NDEKTSV4RRFFQ69G5M52".to_owned(),
                principal: "user:ops".to_owned(),
                channel: Some("cli".to_owned()),
                session_id: None,
                source: MemorySource::Manual,
                content_text: "follow up memory projection requiring reconciliation".to_owned(),
                tags: Vec::new(),
                confidence: Some(0.88),
                ttl_unix_ms: None,
            })
            .expect("second memory item should be indexed in journal");

        let drift = external_index
            .detect_drift(&store, 3_000)
            .expect("stale checkpoint should produce drift");
        assert_eq!(drift.memory_drift, 1);
        assert!(drift.reconciliation_required);

        let reconciliation = external_index
            .reconcile(&store, 1, 4_000)
            .expect("reconciliation should run journal-derived backfill");
        assert!(reconciliation.success);
        assert!(reconciliation.drift_before.reconciliation_required);
        assert!(!reconciliation.drift_after.reconciliation_required);
        assert_eq!(reconciliation.indexer.indexed_memory_items, 2);
        assert_eq!(external_index.snapshot().scale_slos.preview_gate_state, "preview_ready");
    }
}
