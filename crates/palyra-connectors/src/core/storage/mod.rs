use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use rusqlite::{Connection, Transaction};
use thiserror::Error;

mod dead_letters;
mod events;
mod instances;
mod outbox;
mod queue_state;
mod records;
mod schema;

#[cfg(test)]
mod tests;

pub use records::{
    ConnectorEventRecord, ConnectorInstanceRecord, ConnectorQueueSnapshot, DeadLetterRecord,
    OutboxEnqueueOutcome, OutboxEntryRecord,
};

#[derive(Debug)]
pub struct ConnectorStore {
    db_path: PathBuf,
    connection: Mutex<Connection>,
}

#[derive(Debug, Error)]
pub enum ConnectorStoreError {
    #[error("sqlite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization failed: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("connector storage lock is poisoned")]
    PoisonedLock,
    #[error("connector storage schema contains unknown connector kind '{0}'")]
    UnknownConnectorKind(String),
    #[error("connector storage schema contains unknown readiness '{0}'")]
    UnknownReadiness(String),
    #[error("connector storage schema contains unknown liveness '{0}'")]
    UnknownLiveness(String),
    #[error("connector storage value overflow while converting '{field}'")]
    ValueOverflow { field: &'static str },
    #[error("connector record not found: {0}")]
    NotFound(String),
    #[error("outbox entry not found: {0}")]
    OutboxNotFound(i64),
    #[error("dead-letter entry not found: {0}")]
    DeadLetterNotFound(i64),
}

pub(super) const OUTBOX_CLAIM_LEASE_MS: i64 = 60_000;
static OUTBOX_CLAIM_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl ConnectorStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ConnectorStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            }
        }
        let connection = Connection::open(path.as_path())?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let store = Self { db_path: path, connection: Mutex::new(connection) };
        store.initialize_schema()?;
        Ok(store)
    }

    #[must_use]
    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
    }

    fn with_transaction<T, F>(&self, callback: F) -> Result<T, ConnectorStoreError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, ConnectorStoreError>,
    {
        let mut connection =
            self.connection.lock().map_err(|_| ConnectorStoreError::PoisonedLock)?;
        let transaction = connection.transaction()?;
        let output = callback(&transaction)?;
        transaction.commit()?;
        Ok(output)
    }
}

pub(super) fn next_outbox_claim_token(now_unix_ms: i64) -> String {
    let sequence = OUTBOX_CLAIM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("claim-{now_unix_ms}-{sequence}")
}
