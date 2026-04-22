use std::sync::Arc;

use tonic::Status;

use super::{map_memory_store_error, GatewayRuntimeState};
use crate::{
    gateway::current_unix_ms,
    retrieval::{
        ExternalRetrievalDriftReport, ExternalRetrievalIndexerOutcome,
        ExternalRetrievalReconciliationOutcome,
    },
};

impl GatewayRuntimeState {
    #[allow(clippy::result_large_err)]
    pub async fn run_external_retrieval_indexer(
        self: &Arc<Self>,
        batch_size: usize,
    ) -> Result<ExternalRetrievalIndexerOutcome, Status> {
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            state
                .external_retrieval_index
                .run_indexer(&state.journal_store, batch_size, 1, current_unix_ms())
                .map_err(|error| map_memory_store_error("run external retrieval indexer", error))
        })
        .await
        .map_err(|_| Status::internal("external retrieval indexer worker panicked"))?
    }

    #[allow(clippy::result_large_err)]
    pub async fn external_retrieval_drift_report(
        self: &Arc<Self>,
    ) -> Result<ExternalRetrievalDriftReport, Status> {
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            state
                .external_retrieval_index
                .detect_drift(&state.journal_store, current_unix_ms())
                .map_err(|error| map_memory_store_error("detect external retrieval drift", error))
        })
        .await
        .map_err(|_| Status::internal("external retrieval drift worker panicked"))?
    }

    #[allow(clippy::result_large_err)]
    pub async fn reconcile_external_retrieval_index(
        self: &Arc<Self>,
        batch_size: usize,
    ) -> Result<ExternalRetrievalReconciliationOutcome, Status> {
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            state
                .external_retrieval_index
                .reconcile(&state.journal_store, batch_size, current_unix_ms())
                .map_err(|error| {
                    map_memory_store_error("reconcile external retrieval index", error)
                })
        })
        .await
        .map_err(|_| Status::internal("external retrieval reconciliation worker panicked"))?
    }
}
