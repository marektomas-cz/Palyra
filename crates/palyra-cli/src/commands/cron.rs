use crate::*;

pub(crate) fn run_cron(command: CronCommand) -> Result<()> {
    let root_context = app::current_root_context()
        .ok_or_else(|| anyhow!("CLI root context is unavailable for cron command"))?;
    let connection = root_context.resolve_grpc_connection(
        app::ConnectionOverrides::default(),
        app::ConnectionDefaults::USER,
    )?;
    let runtime = build_runtime()?;
    runtime.block_on(run_cron_async(command, connection))
}

pub(crate) async fn run_cron_async(
    command: CronCommand,
    connection: AgentConnection,
) -> Result<()> {
    let mut client =
        cron_v1::cron_service_client::CronServiceClient::connect(connection.grpc_url.clone())
            .await
            .with_context(|| {
                format!("failed to connect gateway gRPC endpoint {}", connection.grpc_url)
            })?;

    match command {
        CronCommand::Status { after, limit, enabled, owner, channel, json } => {
            let list_limit = limit.unwrap_or(25).clamp(1, 100);
            let mut request = Request::new(cron_v1::ListJobsRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                after_job_ulid: after.unwrap_or_default(),
                limit: list_limit,
                enabled,
                owner_principal: owner,
                channel,
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response = client
                .list_jobs(request)
                .await
                .context("failed to call cron ListJobs for status")?
                .into_inner();
            let now_unix_ms = unix_now_ms();
            let due_soon_window_ms = 15_i64 * 60_i64 * 1_000_i64;
            let mut enabled_jobs = 0_u64;
            let mut disabled_jobs = 0_u64;
            let mut overdue_jobs = 0_u64;
            let mut due_soon_jobs = 0_u64;
            let mut running_jobs = 0_u64;
            let mut succeeded_jobs = 0_u64;
            let mut failed_jobs = 0_u64;
            let mut skipped_jobs = 0_u64;
            let mut denied_jobs = 0_u64;
            let mut jobs_payload = Vec::with_capacity(response.jobs.len());
            for job in response.jobs {
                let recent_run =
                    fetch_recent_cron_run(&mut client, &connection, job.job_id.as_ref()).await?;
                let next_run_at_unix_ms = job.next_run_at_unix_ms;
                let overdue =
                    job.enabled && next_run_at_unix_ms > 0 && next_run_at_unix_ms <= now_unix_ms;
                let due_soon = job.enabled
                    && next_run_at_unix_ms > now_unix_ms
                    && next_run_at_unix_ms.saturating_sub(now_unix_ms) <= due_soon_window_ms;
                let late_by_ms = if overdue {
                    Some(now_unix_ms.saturating_sub(next_run_at_unix_ms))
                } else {
                    None
                };
                if job.enabled {
                    enabled_jobs = enabled_jobs.saturating_add(1);
                } else {
                    disabled_jobs = disabled_jobs.saturating_add(1);
                }
                if overdue {
                    overdue_jobs = overdue_jobs.saturating_add(1);
                }
                if due_soon {
                    due_soon_jobs = due_soon_jobs.saturating_add(1);
                }
                if let Some(run) = recent_run.as_ref() {
                    match cron_v1::JobRunStatus::try_from(run.status)
                        .unwrap_or(cron_v1::JobRunStatus::Unspecified)
                    {
                        cron_v1::JobRunStatus::Running => {
                            running_jobs = running_jobs.saturating_add(1);
                        }
                        cron_v1::JobRunStatus::Succeeded => {
                            succeeded_jobs = succeeded_jobs.saturating_add(1);
                        }
                        cron_v1::JobRunStatus::Failed => {
                            failed_jobs = failed_jobs.saturating_add(1);
                        }
                        cron_v1::JobRunStatus::Skipped => {
                            skipped_jobs = skipped_jobs.saturating_add(1);
                        }
                        cron_v1::JobRunStatus::Denied => {
                            denied_jobs = denied_jobs.saturating_add(1);
                        }
                        cron_v1::JobRunStatus::Accepted | cron_v1::JobRunStatus::Unspecified => {}
                    }
                }
                let last_status = recent_run.as_ref().map(cron_run_status_text);
                jobs_payload.push(json!({
                    "job": cron_job_to_json(&job),
                    "recent_run": recent_run.as_ref().map(cron_run_to_json),
                    "last_status": last_status,
                    "overdue": overdue,
                    "due_soon": due_soon,
                    "late_by_ms": late_by_ms,
                }));
            }
            let summary = json!({
                "total_jobs": enabled_jobs + disabled_jobs,
                "enabled_jobs": enabled_jobs,
                "disabled_jobs": disabled_jobs,
                "overdue_jobs": overdue_jobs,
                "due_soon_jobs": due_soon_jobs,
                "running_jobs": running_jobs,
                "succeeded_jobs": succeeded_jobs,
                "failed_jobs": failed_jobs,
                "skipped_jobs": skipped_jobs,
                "denied_jobs": denied_jobs,
                "evaluated_at_unix_ms": now_unix_ms,
            });
            let json_output = output::preferred_json(json);
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "summary": summary,
                        "jobs": jobs_payload,
                        "next_after_job_ulid": response.next_after_job_ulid,
                    }))
                    .context("failed to serialize cron status JSON output")?
                );
            } else {
                println!(
                    "cron.status total_jobs={} enabled_jobs={} disabled_jobs={} overdue_jobs={} due_soon_jobs={} running_jobs={} succeeded_jobs={} failed_jobs={} skipped_jobs={} denied_jobs={}",
                    enabled_jobs + disabled_jobs,
                    enabled_jobs,
                    disabled_jobs,
                    overdue_jobs,
                    due_soon_jobs,
                    running_jobs,
                    succeeded_jobs,
                    failed_jobs,
                    skipped_jobs,
                    denied_jobs
                );
                for job in jobs_payload {
                    let item = job.get("job");
                    let id = item
                        .and_then(|value| value.get("job_id"))
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let name = item
                        .and_then(|value| value.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let enabled = item
                        .and_then(|value| value.get("enabled"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let next_run_at_unix_ms = item
                        .and_then(|value| value.get("next_run_at_unix_ms"))
                        .and_then(Value::as_i64)
                        .unwrap_or_default();
                    let last_status =
                        job.get("last_status").and_then(Value::as_str).unwrap_or("none");
                    let overdue = job.get("overdue").and_then(Value::as_bool).unwrap_or(false);
                    let due_soon = job.get("due_soon").and_then(Value::as_bool).unwrap_or(false);
                    let late_by_ms = job
                        .get("late_by_ms")
                        .and_then(Value::as_i64)
                        .map_or("none".to_owned(), |v| v.to_string());
                    println!(
                        "cron.job id={} name={} enabled={} next_run_at_unix_ms={} last_status={} overdue={} due_soon={} late_by_ms={}",
                        id, name, enabled, next_run_at_unix_ms, last_status, overdue, due_soon, late_by_ms
                    );
                }
            }
        }
        CronCommand::List { after, limit, enabled, owner, channel, json } => {
            let mut request = Request::new(cron_v1::ListJobsRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                after_job_ulid: after.unwrap_or_default(),
                limit: limit.unwrap_or(100),
                enabled,
                owner_principal: owner,
                channel,
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response = client
                .list_jobs(request)
                .await
                .context("failed to call cron ListJobs")?
                .into_inner();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "jobs": response.jobs.iter().map(cron_job_to_json).collect::<Vec<_>>(),
                        "next_after_job_ulid": response.next_after_job_ulid,
                    }))
                    .context("failed to serialize JSON output")?
                );
            } else {
                println!(
                    "cron.list jobs={} next_after={}",
                    response.jobs.len(),
                    if response.next_after_job_ulid.is_empty() {
                        "none"
                    } else {
                        response.next_after_job_ulid.as_str()
                    }
                );
                for job in response.jobs {
                    let id =
                        job.job_id.as_ref().map(|value| value.ulid.as_str()).unwrap_or("unknown");
                    println!(
                        "cron.job id={} name={} enabled={} owner={} channel={} next_run_at_ms={}",
                        id,
                        job.name,
                        job.enabled,
                        job.owner_principal,
                        job.channel,
                        job.next_run_at_unix_ms
                    );
                }
            }
        }
        CronCommand::Show { id, json } => {
            validate_canonical_id(id.as_str()).context("cron job id must be a canonical ULID")?;
            let mut request = Request::new(cron_v1::GetJobRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                job_id: Some(common_v1::CanonicalId { ulid: id.clone() }),
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response =
                client.get_job(request).await.context("failed to call cron GetJob")?.into_inner();
            let job = response.job.context("cron GetJob returned empty job payload")?;
            if json {
                println!("{}", serde_json::to_string_pretty(&cron_job_to_json(&job))?);
            } else {
                println!(
                    "cron.show id={} name={} enabled={} owner={} channel={} schedule_type={}",
                    id,
                    job.name,
                    job.enabled,
                    job.owner_principal,
                    job.channel,
                    job.schedule.map(|s| s.r#type).unwrap_or_default()
                );
            }
        }
        CronCommand::Add {
            name,
            prompt,
            schedule_type,
            schedule,
            enabled,
            concurrency,
            retry_max_attempts,
            retry_backoff_ms,
            misfire,
            jitter_ms,
            owner,
            channel,
            session_key,
            session_label,
            json,
        } => {
            let schedule = build_cron_schedule(schedule_type, schedule)?;
            let mut request = Request::new(cron_v1::CreateJobRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                name,
                prompt,
                owner_principal: owner.unwrap_or_else(|| connection.principal.clone()),
                channel: channel.unwrap_or_else(|| "system:cron".to_owned()),
                session_key: session_key.unwrap_or_default(),
                session_label: session_label.unwrap_or_default(),
                schedule: Some(schedule),
                enabled,
                concurrency_policy: cron_concurrency_to_proto(concurrency),
                retry_policy: Some(cron_v1::RetryPolicy {
                    max_attempts: retry_max_attempts.max(1),
                    backoff_ms: retry_backoff_ms.max(1),
                }),
                misfire_policy: cron_misfire_to_proto(misfire),
                jitter_ms,
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response = client
                .create_job(request)
                .await
                .context("failed to call cron CreateJob")?
                .into_inner();
            let job = response.job.context("cron CreateJob returned empty job payload")?;
            if json {
                println!("{}", serde_json::to_string_pretty(&cron_job_to_json(&job))?);
            } else {
                let id = job.job_id.as_ref().map(|value| value.ulid.as_str()).unwrap_or("unknown");
                println!(
                    "cron.add id={} name={} enabled={} owner={} channel={}",
                    id, job.name, job.enabled, job.owner_principal, job.channel
                );
            }
        }
        CronCommand::Update {
            id,
            name,
            prompt,
            schedule_type,
            schedule,
            enabled,
            concurrency,
            retry_max_attempts,
            retry_backoff_ms,
            misfire,
            jitter_ms,
            owner,
            channel,
            session_key,
            session_label,
            json,
        } => {
            validate_canonical_id(id.as_str()).context("cron job id must be a canonical ULID")?;
            let schedule = match (schedule_type, schedule) {
                (Some(schedule_type), Some(schedule)) => {
                    Some(build_cron_schedule(schedule_type, schedule)?)
                }
                (None, None) => None,
                _ => {
                    unreachable!("clap requires schedule-type and schedule to be provided together")
                }
            };
            let retry_policy = match (retry_max_attempts, retry_backoff_ms) {
                (Some(max_attempts), Some(backoff_ms)) => Some(cron_v1::RetryPolicy {
                    max_attempts: max_attempts.max(1),
                    backoff_ms: backoff_ms.max(1),
                }),
                (None, None) => None,
                _ => unreachable!("clap requires retry policy fields to be provided together"),
            };
            let has_changes = name.is_some()
                || prompt.is_some()
                || owner.is_some()
                || channel.is_some()
                || session_key.is_some()
                || session_label.is_some()
                || schedule.is_some()
                || enabled.is_some()
                || concurrency.is_some()
                || retry_policy.is_some()
                || misfire.is_some()
                || jitter_ms.is_some();
            if !has_changes {
                return Err(anyhow!("cron update requires at least one field to change"));
            }

            let mut request = Request::new(cron_v1::UpdateJobRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                job_id: Some(common_v1::CanonicalId { ulid: id }),
                name,
                prompt,
                owner_principal: owner,
                channel,
                session_key,
                session_label,
                schedule,
                enabled,
                concurrency_policy: concurrency.map(cron_concurrency_to_proto),
                retry_policy,
                misfire_policy: misfire.map(cron_misfire_to_proto),
                jitter_ms,
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response = client
                .update_job(request)
                .await
                .context("failed to call cron UpdateJob")?
                .into_inner();
            let job = response.job.context("cron UpdateJob returned empty job payload")?;
            if json {
                println!("{}", serde_json::to_string_pretty(&cron_job_to_json(&job))?);
            } else {
                println!(
                    "cron.update id={} enabled={} owner={} channel={}",
                    job.job_id.map(|value| value.ulid).unwrap_or_default(),
                    job.enabled,
                    job.owner_principal,
                    job.channel
                );
            }
        }
        CronCommand::Enable { id, json } => {
            let response = update_cron_enabled(&mut client, &connection, id, true).await?;
            if json {
                let job = response.job.context("cron UpdateJob returned empty job payload")?;
                println!("{}", serde_json::to_string_pretty(&cron_job_to_json(&job))?);
            } else {
                let job = response.job.context("cron UpdateJob returned empty job payload")?;
                println!(
                    "cron.enable id={} enabled={}",
                    job.job_id.map(|value| value.ulid).unwrap_or_default(),
                    job.enabled
                );
            }
        }
        CronCommand::Disable { id, json } => {
            let response = update_cron_enabled(&mut client, &connection, id, false).await?;
            if json {
                let job = response.job.context("cron UpdateJob returned empty job payload")?;
                println!("{}", serde_json::to_string_pretty(&cron_job_to_json(&job))?);
            } else {
                let job = response.job.context("cron UpdateJob returned empty job payload")?;
                println!(
                    "cron.disable id={} enabled={}",
                    job.job_id.map(|value| value.ulid).unwrap_or_default(),
                    job.enabled
                );
            }
        }
        CronCommand::RunNow { id, json } => {
            validate_canonical_id(id.as_str()).context("cron job id must be a canonical ULID")?;
            let mut request = Request::new(cron_v1::RunJobNowRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                job_id: Some(common_v1::CanonicalId { ulid: id.clone() }),
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response = client
                .run_job_now(request)
                .await
                .context("failed to call cron RunJobNow")?
                .into_inner();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "run_id": response.run_id.map(|value| value.ulid),
                        "status": response.status,
                        "message": response.message,
                    }))?
                );
            } else {
                println!(
                    "cron.run_now id={} run_id={} status={} message={}",
                    id,
                    response.run_id.map(|value| value.ulid).unwrap_or_default(),
                    response.status,
                    response.message
                );
            }
        }
        CronCommand::Delete { id, json } => {
            validate_canonical_id(id.as_str()).context("cron job id must be a canonical ULID")?;
            let mut request = Request::new(cron_v1::DeleteJobRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                job_id: Some(common_v1::CanonicalId { ulid: id.clone() }),
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response = client
                .delete_job(request)
                .await
                .context("failed to call cron DeleteJob")?
                .into_inner();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "deleted": response.deleted,
                    }))?
                );
            } else {
                println!("cron.delete id={} deleted={}", id, response.deleted);
            }
        }
        CronCommand::Logs { id, after, limit, json } => {
            validate_canonical_id(id.as_str()).context("cron job id must be a canonical ULID")?;
            let mut request = Request::new(cron_v1::ListJobRunsRequest {
                v: CANONICAL_PROTOCOL_MAJOR,
                job_id: Some(common_v1::CanonicalId { ulid: id.clone() }),
                after_run_ulid: after.unwrap_or_default(),
                limit: limit.unwrap_or(100),
            });
            inject_run_stream_metadata(request.metadata_mut(), &connection)?;
            let response = client
                .list_job_runs(request)
                .await
                .context("failed to call cron ListJobRuns")?
                .into_inner();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "runs": response.runs.iter().map(cron_run_to_json).collect::<Vec<_>>(),
                        "next_after_run_ulid": response.next_after_run_ulid,
                    }))?
                );
            } else {
                println!(
                    "cron.logs id={} runs={} next_after={}",
                    id,
                    response.runs.len(),
                    if response.next_after_run_ulid.is_empty() {
                        "none"
                    } else {
                        response.next_after_run_ulid.as_str()
                    }
                );
                for run in response.runs {
                    println!(
                        "cron.run run_id={} status={} started_at_ms={} finished_at_ms={} tool_calls={} tool_denies={}",
                        run.run_id.map(|value| value.ulid).unwrap_or_default(),
                        run.status,
                        run.started_at_unix_ms,
                        run.finished_at_unix_ms,
                        run.tool_calls,
                        run.tool_denies
                    );
                }
            }
        }
    }

    std::io::stdout().flush().context("stdout flush failed")
}

async fn fetch_recent_cron_run(
    client: &mut cron_v1::cron_service_client::CronServiceClient<tonic::transport::Channel>,
    connection: &AgentConnection,
    job_id: Option<&common_v1::CanonicalId>,
) -> Result<Option<cron_v1::JobRun>> {
    let Some(job_id) = job_id.cloned() else {
        return Ok(None);
    };
    let mut request = Request::new(cron_v1::ListJobRunsRequest {
        v: CANONICAL_PROTOCOL_MAJOR,
        job_id: Some(job_id),
        after_run_ulid: String::new(),
        limit: 1,
    });
    inject_run_stream_metadata(request.metadata_mut(), connection)?;
    let response = client
        .list_job_runs(request)
        .await
        .context("failed to call cron ListJobRuns for status inspection")?
        .into_inner();
    Ok(response.runs.into_iter().next())
}

fn cron_run_status_text(run: &cron_v1::JobRun) -> &'static str {
    match cron_v1::JobRunStatus::try_from(run.status).unwrap_or(cron_v1::JobRunStatus::Unspecified)
    {
        cron_v1::JobRunStatus::Accepted => "accepted",
        cron_v1::JobRunStatus::Running => "running",
        cron_v1::JobRunStatus::Succeeded => "succeeded",
        cron_v1::JobRunStatus::Failed => "failed",
        cron_v1::JobRunStatus::Skipped => "skipped",
        cron_v1::JobRunStatus::Denied => "denied",
        cron_v1::JobRunStatus::Unspecified => "unspecified",
    }
}
