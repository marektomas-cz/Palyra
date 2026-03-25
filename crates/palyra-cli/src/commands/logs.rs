use crate::*;

#[derive(Debug, Serialize)]
struct JournalLogRecord {
    seq: i64,
    event_id: String,
    kind: i32,
    timestamp_unix_ms: i64,
    message: Option<String>,
}

pub(crate) fn run_logs(
    db_path: Option<String>,
    lines: usize,
    follow: bool,
    poll_interval_ms: u64,
) -> Result<()> {
    let lines = lines.clamp(1, 500);
    let db_path = resolve_daemon_journal_db_path(db_path)?;
    ensure_journal_db_exists(db_path.as_path())?;
    let mut last_seq = emit_recent_records(db_path.as_path(), lines)?;
    if !follow {
        return Ok(());
    }

    let sleep_duration = Duration::from_millis(poll_interval_ms.clamp(250, 30_000));
    loop {
        thread::sleep(sleep_duration);
        last_seq = emit_follow_records(db_path.as_path(), last_seq)?;
    }
}

fn emit_recent_records(db_path: &Path, limit: usize) -> Result<i64> {
    let connection = Connection::open(db_path)
        .with_context(|| format!("failed to open journal database {}", db_path.display()))?;
    let mut statement = connection.prepare(
        "SELECT seq, event_ulid, kind, timestamp_unix_ms, payload_json
         FROM journal_events
         ORDER BY seq DESC
         LIMIT ?1",
    )?;
    let mut rows = statement.query([limit as i64])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        records.push(read_log_record(row)?);
    }
    records.reverse();
    let mut last_seq = 0_i64;
    for record in records {
        last_seq = record.seq;
        emit_log_record(&record)?;
    }
    Ok(last_seq)
}

fn emit_follow_records(db_path: &Path, after_seq: i64) -> Result<i64> {
    let connection = Connection::open(db_path)
        .with_context(|| format!("failed to open journal database {}", db_path.display()))?;
    let mut statement = connection.prepare(
        "SELECT seq, event_ulid, kind, timestamp_unix_ms, payload_json
         FROM journal_events
         WHERE seq > ?1
         ORDER BY seq ASC",
    )?;
    let mut rows = statement.query([after_seq])?;
    let mut last_seq = after_seq;
    while let Some(row) = rows.next()? {
        let record = read_log_record(row)?;
        last_seq = record.seq;
        emit_log_record(&record)?;
    }
    Ok(last_seq)
}

fn read_log_record(row: &rusqlite::Row<'_>) -> Result<JournalLogRecord> {
    let payload_json: String = row.get(4)?;
    Ok(JournalLogRecord {
        seq: row.get(0)?,
        event_id: row.get(1)?,
        kind: row.get(2)?,
        timestamp_unix_ms: row.get(3)?,
        message: extract_support_bundle_error_message(payload_json.as_str()),
    })
}

fn emit_log_record(record: &JournalLogRecord) -> Result<()> {
    let root_context = app::current_root_context()
        .ok_or_else(|| anyhow!("CLI root context is unavailable for logs command"))?;
    if root_context.prefers_json() {
        return output::print_json_pretty(record, "failed to encode logs output as JSON");
    }
    if root_context.prefers_ndjson() {
        return output::print_json_line(record, "failed to encode logs output as NDJSON");
    }
    println!(
        "logs.event seq={} event_id={} kind={} timestamp_unix_ms={} message={}",
        record.seq,
        record.event_id,
        record.kind,
        record.timestamp_unix_ms,
        record.message.as_deref().unwrap_or("none")
    );
    std::io::stdout().flush().context("stdout flush failed")
}
