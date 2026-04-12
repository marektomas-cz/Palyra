use rusqlite::{params, OptionalExtension};

use super::super::protocol::ConnectorQueueDepth;
use super::records::to_queue_depth;
use super::{ConnectorQueueSnapshot, ConnectorStore, ConnectorStoreError};

impl ConnectorStore {
    pub fn queue_depth(
        &self,
        connector_id: &str,
    ) -> Result<ConnectorQueueDepth, ConnectorStoreError> {
        let snapshot = self.queue_snapshot(connector_id, 0)?;
        Ok(to_queue_depth(&snapshot))
    }

    pub fn queue_snapshot(
        &self,
        connector_id: &str,
        now_unix_ms: i64,
    ) -> Result<ConnectorQueueSnapshot, ConnectorStoreError> {
        let connection = self.connection.lock().map_err(|_| ConnectorStoreError::PoisonedLock)?;
        let pending_outbox: i64 = connection.query_row(
            "SELECT COUNT(*) FROM outbox WHERE connector_id = ?1 AND status = 'pending'",
            params![connector_id],
            |row| row.get(0),
        )?;
        let due_outbox: i64 = connection.query_row(
            r#"
            SELECT COUNT(*)
            FROM outbox
            WHERE connector_id = ?1
              AND status = 'pending'
              AND next_attempt_unix_ms <= ?2
              AND claim_expires_unix_ms <= ?2
            "#,
            params![connector_id, now_unix_ms],
            |row| row.get(0),
        )?;
        let claimed_outbox: i64 = connection.query_row(
            r#"
            SELECT COUNT(*)
            FROM outbox
            WHERE connector_id = ?1
              AND status = 'pending'
              AND claim_expires_unix_ms > ?2
            "#,
            params![connector_id, now_unix_ms],
            |row| row.get(0),
        )?;
        let dead_letters: i64 = connection.query_row(
            "SELECT COUNT(*) FROM dead_letters WHERE connector_id = ?1",
            params![connector_id],
            |row| row.get(0),
        )?;
        let next_attempt_unix_ms = connection.query_row(
            r#"
                SELECT MIN(next_attempt_unix_ms)
                FROM outbox
                WHERE connector_id = ?1
                  AND status = 'pending'
                "#,
            params![connector_id],
            |row| row.get::<_, Option<i64>>(0),
        )?;
        let oldest_pending_created_at_unix_ms = connection.query_row(
            r#"
                SELECT MIN(created_at_unix_ms)
                FROM outbox
                WHERE connector_id = ?1
                  AND status = 'pending'
                "#,
            params![connector_id],
            |row| row.get::<_, Option<i64>>(0),
        )?;
        let latest_dead_letter_unix_ms = connection.query_row(
            r#"
                SELECT MAX(created_at_unix_ms)
                FROM dead_letters
                WHERE connector_id = ?1
                "#,
            params![connector_id],
            |row| row.get::<_, Option<i64>>(0),
        )?;
        let queue_state = connection
            .query_row(
                r#"
                SELECT paused, pause_reason, updated_at_unix_ms
                FROM connector_queue_state
                WHERE connector_id = ?1
                "#,
                params![connector_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? != 0,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        Ok(ConnectorQueueSnapshot {
            pending_outbox: u64::try_from(pending_outbox).unwrap_or(0),
            due_outbox: u64::try_from(due_outbox).unwrap_or(0),
            claimed_outbox: u64::try_from(claimed_outbox).unwrap_or(0),
            dead_letters: u64::try_from(dead_letters).unwrap_or(0),
            next_attempt_unix_ms,
            oldest_pending_created_at_unix_ms,
            latest_dead_letter_unix_ms,
            paused: queue_state.as_ref().is_some_and(|(paused, _, _)| *paused),
            pause_reason: queue_state.as_ref().and_then(|(_, reason, _)| reason.clone()),
            pause_updated_at_unix_ms: queue_state.as_ref().map(|(_, _, updated_at)| *updated_at),
        })
    }

    pub fn set_queue_paused(
        &self,
        connector_id: &str,
        paused: bool,
        reason: Option<&str>,
        now_unix_ms: i64,
    ) -> Result<(), ConnectorStoreError> {
        self.with_transaction(|transaction| {
            let connector_exists: Option<i64> = transaction
                .query_row(
                    "SELECT 1 FROM connector_instances WHERE connector_id = ?1",
                    params![connector_id],
                    |row| row.get(0),
                )
                .optional()?;
            if connector_exists.is_none() {
                return Err(ConnectorStoreError::NotFound(connector_id.to_owned()));
            }

            transaction.execute(
                r#"
                INSERT INTO connector_queue_state (
                    connector_id, paused, pause_reason, updated_at_unix_ms
                )
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(connector_id) DO UPDATE SET
                    paused = excluded.paused,
                    pause_reason = excluded.pause_reason,
                    updated_at_unix_ms = excluded.updated_at_unix_ms
                "#,
                params![connector_id, if paused { 1_i64 } else { 0_i64 }, reason, now_unix_ms,],
            )?;
            Ok(())
        })?;
        Ok(())
    }
}
