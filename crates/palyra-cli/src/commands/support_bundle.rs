use crate::{output::support_bundle as support_bundle_output, *};

pub(crate) fn run_support_bundle(command: SupportBundleCommand) -> Result<()> {
    match command {
        SupportBundleCommand::Export { output, max_bytes, journal_hash_limit, error_limit } => {
            run_support_bundle_export(output, max_bytes, journal_hash_limit, error_limit)
        }
        SupportBundleCommand::ReplayExport { run_id, output, journal_db, max_events } => {
            run_replay_export(run_id, output, journal_db, max_events)
        }
        SupportBundleCommand::ReplayImport { input, output_dir } => {
            run_replay_import(input, output_dir)
        }
        SupportBundleCommand::ReplayRun { input, diff_output } => {
            run_replay_run(input, diff_output)
        }
        SupportBundleCommand::ReplayBaseline { input, output } => {
            run_replay_baseline(input, output)
        }
    }
}

fn run_support_bundle_export(
    output: Option<String>,
    max_bytes: usize,
    journal_hash_limit: usize,
    error_limit: usize,
) -> Result<()> {
    if max_bytes < 2_048 {
        anyhow::bail!("support-bundle max-bytes must be at least 2048");
    }
    let generated_at_unix_ms = now_unix_ms_i64()?;
    let checks = build_doctor_checks();
    let doctor = build_doctor_report(checks.as_slice())?;
    let output_path = resolve_support_bundle_output_path(output, generated_at_unix_ms);

    let build = build_metadata();
    let diagnostics = build_support_bundle_diagnostics_snapshot();
    let profile = app::current_root_context().and_then(|context| context.active_profile_context());
    let mut bundle = SupportBundle {
        schema_version: 1,
        generated_at_unix_ms,
        profile,
        build: SupportBundleBuildSnapshot {
            version: build.version.to_owned(),
            git_hash: build.git_hash.to_owned(),
            build_profile: build.build_profile.to_owned(),
        },
        platform: SupportBundlePlatformSnapshot {
            os: std::env::consts::OS.to_owned(),
            family: std::env::consts::FAMILY.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        },
        doctor,
        recovery: Some(commands::doctor::build_doctor_support_bundle_value()?),
        config: build_support_bundle_config_snapshot(),
        observability: build_support_bundle_observability_snapshot(&diagnostics),
        triage: build_support_bundle_triage_snapshot(),
        replay: build_support_bundle_replay_snapshot(),
        diagnostics,
        journal: build_support_bundle_journal_snapshot(journal_hash_limit, error_limit),
        truncated: false,
        warnings: Vec::new(),
    };

    let encoded = encode_support_bundle_with_cap(&mut bundle, max_bytes)?;
    if let Some(parent) = output_path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create support-bundle directory {}", parent.display())
        })?;
    }
    fs::write(output_path.as_path(), encoded.as_slice())
        .with_context(|| format!("failed to write support bundle {}", output_path.display()))?;
    support_bundle_output::emit_export(&output_path, encoded.len(), &bundle)
}

fn run_replay_export(
    run_id: String,
    output: String,
    journal_db: Option<String>,
    max_events: usize,
) -> Result<()> {
    if max_events == 0 || max_events > 4_096 {
        anyhow::bail!("support-bundle replay-export --max-events must be in range 1..=4096");
    }
    let output_path = PathBuf::from(output);
    let bundle = build_replay_bundle_from_journal(run_id.as_str(), journal_db, max_events)?;
    let encoded = canonical_replay_bundle_bytes(&bundle)?;
    write_replay_artifact(output_path.as_path(), encoded.as_slice())?;
    println!(
        "support_bundle.replay_export path={} run_id={} bytes={} tape_events={} canonical_sha256={}",
        output_path.display(),
        bundle.source.run_id,
        encoded.len(),
        bundle.tape_events.len(),
        bundle
            .integrity
            .canonical_sha256
            .as_deref()
            .unwrap_or("<missing>")
    );
    std::io::stdout().flush().context("stdout flush failed")
}

fn run_replay_import(input: String, output_dir: String) -> Result<()> {
    let input_path = PathBuf::from(input);
    let bytes = fs::read(input_path.as_path())
        .with_context(|| format!("failed to read replay bundle {}", input_path.display()))?;
    let bundle = parse_replay_bundle(bytes.as_slice())?;
    let report = replay_bundle_offline(&bundle);
    ensure_replay_report_passed(&report)?;
    let output_dir = PathBuf::from(output_dir);
    fs::create_dir_all(output_dir.as_path()).with_context(|| {
        format!("failed to create replay import directory {}", output_dir.display())
    })?;
    let hash =
        bundle.integrity.canonical_sha256.clone().unwrap_or_else(|| sha256_hex(bytes.as_slice()));
    let output_path =
        output_dir.join(format!("{}.replay.json", hash.chars().take(16).collect::<String>()));
    write_replay_artifact(output_path.as_path(), bytes.as_slice())?;
    println!(
        "support_bundle.replay_import path={} status=passed canonical_sha256={}",
        output_path.display(),
        hash
    );
    std::io::stdout().flush().context("stdout flush failed")
}

fn run_replay_run(input: String, diff_output: Option<String>) -> Result<()> {
    let input_path = PathBuf::from(input);
    let bytes = fs::read(input_path.as_path())
        .with_context(|| format!("failed to read replay bundle {}", input_path.display()))?;
    let bundle = parse_replay_bundle(bytes.as_slice())?;
    let report = replay_bundle_offline(&bundle);
    if let Some(output) = diff_output {
        let output_path = PathBuf::from(output);
        let encoded =
            serde_json::to_vec_pretty(&report).context("failed to encode replay diff report")?;
        write_replay_artifact(output_path.as_path(), encoded.as_slice())?;
    }
    println!(
        "support_bundle.replay_run status={:?} diffs={} checked_categories={} validation_issues={}",
        report.status,
        report.diffs.len(),
        report.checked_categories.len(),
        report.validation.issues.len()
    );
    ensure_replay_report_passed(&report)?;
    std::io::stdout().flush().context("stdout flush failed")
}

fn run_replay_baseline(input: String, output: String) -> Result<()> {
    let input_path = PathBuf::from(input);
    let bytes = fs::read(input_path.as_path())
        .with_context(|| format!("failed to read replay bundle {}", input_path.display()))?;
    let mut bundle = parse_replay_bundle(bytes.as_slice())?;
    let report = replay_bundle_offline(&bundle);
    ensure_replay_report_passed(&report)?;
    finalize_replay_bundle(&mut bundle)?;
    let output_path = PathBuf::from(output);
    let encoded = canonical_replay_bundle_bytes(&bundle)?;
    write_replay_artifact(output_path.as_path(), encoded.as_slice())?;
    println!(
        "support_bundle.replay_baseline path={} canonical_sha256={}",
        output_path.display(),
        bundle.integrity.canonical_sha256.as_deref().unwrap_or("<missing>")
    );
    std::io::stdout().flush().context("stdout flush failed")
}

fn build_replay_bundle_from_journal(
    run_id: &str,
    journal_db: Option<String>,
    max_events: usize,
) -> Result<ReplayBundle> {
    let db_path = resolve_daemon_journal_db_path(journal_db)?;
    let connection = Connection::open(db_path.as_path())
        .with_context(|| format!("failed to open journal database {}", db_path.display()))?;
    let run = read_replay_journal_run(&connection, run_id)?
        .with_context(|| format!("orchestrator run not found: {run_id}"))?;
    let tape_events = read_replay_journal_tape(&connection, run_id, max_events)?;
    let truncated = tape_events.len() > max_events;
    let tape_events = tape_events.into_iter().take(max_events).collect::<Vec<_>>();
    let mut config_snapshot = json!({
        "config": build_support_bundle_config_snapshot(),
        "contract": replay_contract_snapshot(),
        "journal": {
            "db_path": db_path.to_string_lossy(),
            "source": "sqlite_orchestrator_tape",
        }
    });
    redact_json_value_tree(&mut config_snapshot, None);

    build_replay_bundle(ReplayBundleBuildInput {
        generated_at_unix_ms: now_unix_ms_i64()?,
        source: ReplaySource {
            product: "palyra".to_owned(),
            run_id: run.run_id.clone(),
            session_id: Some(run.session_id.clone()),
            origin_kind: run.origin_kind.clone(),
            schema_policy: "reject_future_schema_versions_additive_backward_compat".to_owned(),
        },
        capture: ReplayCaptureMetadata {
            captured_at_unix_ms: now_unix_ms_i64()?,
            capture_mode: "cli_support_bundle_replay_export".to_owned(),
            max_events_per_run: max_events,
            truncated,
            inline_sections: vec![
                "run".to_owned(),
                "config_snapshot".to_owned(),
                "tape_events".to_owned(),
                "tool_exchanges".to_owned(),
                "http_exchanges".to_owned(),
                "approvals".to_owned(),
                "expected".to_owned(),
            ],
            referenced_sections: vec![
                "large_binary_artifacts".to_owned(),
                "workspace_files".to_owned(),
                "journal_events_outside_run".to_owned(),
            ],
            warnings: if truncated {
                vec![format!("tape truncated at {max_events} events for replay bundle export")]
            } else {
                Vec::new()
            },
        },
        run: ReplayRunSnapshot {
            state: run.state,
            principal: run.principal,
            device_id: run.device_id,
            channel: run.channel,
            normalized_user_input: extract_replay_user_input(run.parameter_delta_json.as_deref()),
            prompt_tokens: run.prompt_tokens,
            completion_tokens: run.completion_tokens,
            total_tokens: run.total_tokens,
            last_error: run.last_error,
            parent_run_id: run.parent_run_id,
            origin_run_id: run.origin_run_id,
            parameter_delta: run
                .parameter_delta_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok()),
        },
        config_snapshot,
        tape_events,
        artifact_refs: Vec::<ReplayArtifactRef>::new(),
    })
}

fn write_replay_artifact(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create replay artifact directory {}", parent.display())
        })?;
    }
    fs::write(path, bytes)
        .with_context(|| format!("failed to write replay artifact {}", path.display()))
}

#[derive(Debug)]
struct ReplayJournalRunRow {
    run_id: String,
    session_id: String,
    state: String,
    principal: String,
    device_id: String,
    channel: Option<String>,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    last_error: Option<String>,
    origin_kind: String,
    origin_run_id: Option<String>,
    parent_run_id: Option<String>,
    parameter_delta_json: Option<String>,
}

fn read_replay_journal_run(
    connection: &Connection,
    run_id: &str,
) -> Result<Option<ReplayJournalRunRow>> {
    connection
        .query_row(
            r#"
                SELECT
                    runs.run_ulid,
                    runs.session_ulid,
                    runs.state,
                    sessions.principal,
                    sessions.device_id,
                    sessions.channel,
                    runs.prompt_tokens,
                    runs.completion_tokens,
                    runs.total_tokens,
                    runs.last_error,
                    runs.origin_kind,
                    runs.origin_run_ulid,
                    runs.parent_run_ulid,
                    runs.parameter_delta_json
                FROM orchestrator_runs AS runs
                INNER JOIN orchestrator_sessions AS sessions
                    ON sessions.session_ulid = runs.session_ulid
                WHERE runs.run_ulid = ?1
            "#,
            rusqlite::params![run_id],
            |row| {
                let prompt_tokens: i64 = row.get(6)?;
                let completion_tokens: i64 = row.get(7)?;
                let total_tokens: i64 = row.get(8)?;
                Ok(ReplayJournalRunRow {
                    run_id: row.get(0)?,
                    session_id: row.get(1)?,
                    state: row.get(2)?,
                    principal: row.get(3)?,
                    device_id: row.get(4)?,
                    channel: row.get(5)?,
                    prompt_tokens: prompt_tokens.max(0) as u64,
                    completion_tokens: completion_tokens.max(0) as u64,
                    total_tokens: total_tokens.max(0) as u64,
                    last_error: row.get(9)?,
                    origin_kind: row.get(10)?,
                    origin_run_id: row.get(11)?,
                    parent_run_id: row.get(12)?,
                    parameter_delta_json: row.get(13)?,
                })
            },
        )
        .optional()
        .context("failed to read orchestrator run for replay export")
}

fn read_replay_journal_tape(
    connection: &Connection,
    run_id: &str,
    max_events: usize,
) -> Result<Vec<ReplayTapeEvent>> {
    let mut statement = connection
        .prepare(
            r#"
                SELECT seq, event_type, payload_json
                FROM orchestrator_tape
                WHERE run_ulid = ?1
                ORDER BY seq ASC
                LIMIT ?2
            "#,
        )
        .context("failed to prepare replay tape query")?;
    let rows = statement.query_map(rusqlite::params![run_id, (max_events + 1) as i64], |row| {
        let seq: i64 = row.get(0)?;
        let event_type: String = row.get(1)?;
        let payload_json: String = row.get(2)?;
        let payload = serde_json::from_str::<Value>(payload_json.as_str()).unwrap_or_else(|_| {
            json!({
                "raw": payload_json,
            })
        });
        Ok(ReplayTapeEvent { seq, event_type, payload })
    })?;
    let mut events = Vec::new();
    for row in rows {
        events.push(row.context("failed to read replay tape event")?);
    }
    Ok(events)
}

fn extract_replay_user_input(parameter_delta_json: Option<&str>) -> Option<Value> {
    let value = parameter_delta_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok())?;
    value.get("user_input").or_else(|| value.get("input")).or_else(|| value.get("prompt")).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use palyra_common::replay_bundle::{replay_bundle_offline, ReplayRunStatus};

    #[test]
    fn replay_export_from_journal_redacts_and_replays_offline() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let db_path = temp.path().join("journal.sqlite3");
        let connection = Connection::open(db_path.as_path()).expect("sqlite should open");
        connection
            .execute_batch(
                r#"
                CREATE TABLE orchestrator_sessions (
                    session_ulid TEXT PRIMARY KEY,
                    principal TEXT NOT NULL,
                    device_id TEXT NOT NULL,
                    channel TEXT
                );
                CREATE TABLE orchestrator_runs (
                    run_ulid TEXT PRIMARY KEY,
                    session_ulid TEXT NOT NULL,
                    state TEXT NOT NULL,
                    prompt_tokens INTEGER NOT NULL,
                    completion_tokens INTEGER NOT NULL,
                    total_tokens INTEGER NOT NULL,
                    last_error TEXT,
                    origin_kind TEXT NOT NULL,
                    origin_run_ulid TEXT,
                    parent_run_ulid TEXT,
                    parameter_delta_json TEXT
                );
                CREATE TABLE orchestrator_tape (
                    run_ulid TEXT NOT NULL,
                    seq INTEGER NOT NULL,
                    event_type TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    PRIMARY KEY (run_ulid, seq)
                );
                "#,
            )
            .expect("schema should initialize");
        connection
            .execute(
                "INSERT INTO orchestrator_sessions (session_ulid, principal, device_id, channel) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    "01ARZ3NDEKTSV4RRFFQ69G5FB1",
                    "user:ops",
                    "device:local",
                    "cli",
                ],
            )
            .expect("session should insert");
        connection
            .execute(
                "INSERT INTO orchestrator_runs (
                    run_ulid, session_ulid, state, prompt_tokens, completion_tokens, total_tokens,
                    last_error, origin_kind, origin_run_ulid, parent_run_ulid, parameter_delta_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    "01ARZ3NDEKTSV4RRFFQ69G5FB2",
                    "01ARZ3NDEKTSV4RRFFQ69G5FB1",
                    "completed",
                    12_i64,
                    4_i64,
                    16_i64,
                    Option::<String>::None,
                    "run_stream",
                    Option::<String>::None,
                    Option::<String>::None,
                    json!({ "user_input": { "text": "fetch https://example.test?token=raw" } })
                        .to_string(),
                ],
            )
            .expect("run should insert");
        connection
            .execute(
                "INSERT INTO orchestrator_tape (run_ulid, seq, event_type, payload_json) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    "01ARZ3NDEKTSV4RRFFQ69G5FB2",
                    0_i64,
                    "tool_proposal",
                    json!({
                        "proposal_id": "01ARZ3NDEKTSV4RRFFQ69G5FB3",
                        "tool_name": "palyra.http.fetch",
                        "input_json": {
                            "url": "https://example.test/callback?access_token=raw&mode=ok",
                            "headers": { "authorization": "Bearer raw" }
                        }
                    })
                    .to_string(),
                ],
            )
            .expect("proposal should insert");
        connection
            .execute(
                "INSERT INTO orchestrator_tape (run_ulid, seq, event_type, payload_json) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    "01ARZ3NDEKTSV4RRFFQ69G5FB2",
                    1_i64,
                    "tool_result",
                    json!({
                        "proposal_id": "01ARZ3NDEKTSV4RRFFQ69G5FB3",
                        "success": true,
                        "output_json": { "status": 200 },
                        "error": "",
                    })
                    .to_string(),
                ],
            )
            .expect("result should insert");

        let bundle = build_replay_bundle_from_journal(
            "01ARZ3NDEKTSV4RRFFQ69G5FB2",
            Some(db_path.to_string_lossy().into_owned()),
            128,
        )
        .expect("replay bundle should export from journal");
        let encoded = serde_json::to_string(&bundle).expect("bundle should serialize");
        assert!(!encoded.contains("access_token=raw"));
        assert!(!encoded.contains("Bearer raw"));
        assert_eq!(replay_bundle_offline(&bundle).status, ReplayRunStatus::Passed);
    }
}
