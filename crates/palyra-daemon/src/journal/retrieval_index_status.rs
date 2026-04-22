use serde::Serialize;

use super::{JournalError, JournalStore};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceRetrievalIndexStatus {
    pub chunk_count: u64,
    pub indexed_chunk_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_chunk_created_at_unix_ms: Option<i64>,
}

impl JournalStore {
    pub fn workspace_retrieval_index_status(
        &self,
    ) -> Result<WorkspaceRetrievalIndexStatus, JournalError> {
        let guard = self.connection.lock().map_err(|_| JournalError::LockPoisoned)?;
        let (chunk_count, indexed_chunk_count, latest_chunk_created_at_unix_ms) = guard.query_row(
            r#"
                SELECT
                    COUNT(chunks.chunk_ulid),
                    COUNT(vectors.chunk_ulid),
                    MAX(chunks.created_at_unix_ms)
                FROM workspace_document_chunks AS chunks
                INNER JOIN workspace_documents AS documents
                    ON documents.document_ulid = chunks.document_ulid
                LEFT JOIN workspace_document_chunk_vectors AS vectors
                    ON vectors.chunk_ulid = chunks.chunk_ulid
                WHERE chunks.is_latest = 1 AND documents.state = 'active'
            "#,
            [],
            |row| {
                let chunk_count: i64 = row.get(0)?;
                let indexed_chunk_count: i64 = row.get(1)?;
                let latest_chunk_created_at_unix_ms: Option<i64> = row.get(2)?;
                Ok((
                    chunk_count.max(0) as u64,
                    indexed_chunk_count.max(0) as u64,
                    latest_chunk_created_at_unix_ms,
                ))
            },
        )?;
        Ok(WorkspaceRetrievalIndexStatus {
            chunk_count,
            indexed_chunk_count,
            latest_chunk_created_at_unix_ms,
        })
    }
}
