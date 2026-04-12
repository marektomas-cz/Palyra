use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{protocol::ConnectorKind, storage::ConnectorStore};

mod admin;
mod inbound;
mod metrics;
mod outbox;
mod types;

#[cfg(test)]
mod tests;

pub use types::{
    ConnectorAdapter, ConnectorAdapterError, ConnectorRouter, ConnectorRouterError,
    ConnectorSupervisorConfig, ConnectorSupervisorError, DrainOutcome, InboundIngestOutcome,
};

pub struct ConnectorSupervisor {
    store: Arc<ConnectorStore>,
    router: Arc<dyn ConnectorRouter>,
    adapters: HashMap<ConnectorKind, Arc<dyn ConnectorAdapter>>,
    config: ConnectorSupervisorConfig,
}

impl ConnectorSupervisor {
    #[must_use]
    pub fn new(
        store: Arc<ConnectorStore>,
        router: Arc<dyn ConnectorRouter>,
        adapters: Vec<Arc<dyn ConnectorAdapter>>,
        config: ConnectorSupervisorConfig,
    ) -> Self {
        let adapters = adapters
            .into_iter()
            .map(|adapter| (adapter.kind(), adapter))
            .collect::<HashMap<_, _>>();
        Self { store, router, adapters, config }
    }

    #[must_use]
    pub fn store(&self) -> &Arc<ConnectorStore> {
        &self.store
    }
}

pub(super) fn unix_ms_now() -> Result<i64, ConnectorSupervisorError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ConnectorSupervisorError::Clock(error.to_string()))?;
    Ok(now.as_millis().try_into().unwrap_or(i64::MAX))
}
