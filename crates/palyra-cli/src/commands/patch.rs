use crate::*;

pub(crate) fn run_patch(command: PatchCommand) -> Result<()> {
    match command {
        PatchCommand::Apply { stdin, dry_run, json } => {
            if !stdin {
                anyhow::bail!("patch apply requires --stdin");
            }
            let mut patch = String::new();
            std::io::stdin()
                .read_to_string(&mut patch)
                .context("failed to read patch from stdin")?;
            if patch.trim().is_empty() {
                anyhow::bail!("patch from stdin is empty");
            }

            let workspace_root = std::env::current_dir()
                .context("failed to resolve current working directory as workspace root")?;
            let limits = WorkspacePatchLimits::default();
            let redaction_policy = WorkspacePatchRedactionPolicy::default();
            let request = WorkspacePatchRequest {
                patch: patch.clone(),
                dry_run,
                redaction_policy: redaction_policy.clone(),
            };

            match apply_workspace_patch(&[workspace_root], &request, &limits) {
                Ok(outcome) => {
                    if json {
                        let rendered = serde_json::to_string_pretty(&outcome)
                            .context("failed to serialize patch apply output")?;
                        println!("{rendered}");
                    } else {
                        println!(
                            "patch.apply success=true dry_run={} files_touched={} patch_sha256={}",
                            outcome.dry_run,
                            outcome.files_touched.len(),
                            outcome.patch_sha256
                        );
                    }
                    std::io::stdout().flush().context("stdout flush failed")
                }
                Err(error) => {
                    let parse_error = error
                        .parse_location()
                        .map(|(line, column)| json!({ "line": line, "column": column }));
                    let payload = json!({
                        "patch_sha256": compute_patch_sha256(patch.as_str()),
                        "dry_run": dry_run,
                        "rollback_performed": error.rollback_performed(),
                        "redacted_preview": redact_patch_preview(
                            patch.as_str(),
                            &redaction_policy,
                            limits.max_preview_bytes,
                        ),
                        "parse_error": parse_error,
                        "error": error.to_string(),
                    });
                    if json {
                        let rendered = serde_json::to_string_pretty(&payload)
                            .context("failed to serialize patch apply failure output")?;
                        println!("{rendered}");
                    } else {
                        println!(
                            "patch.apply success=false dry_run={} rollback_performed={} error={}",
                            dry_run,
                            error.rollback_performed(),
                            error
                        );
                    }
                    std::io::stdout().flush().context("stdout flush failed")?;
                    anyhow::bail!("patch apply failed: {error}");
                }
            }
        }
    }
}
