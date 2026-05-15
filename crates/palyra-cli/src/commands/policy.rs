use crate::*;

pub(crate) fn run_policy(command: PolicyCommand) -> Result<()> {
    match command {
        PolicyCommand::Explain { principal, action, resource, json } => {
            let request = PolicyRequest { principal, action, resource };
            let policy_context = load_policy_explain_context()?;
            let evaluation = palyra_policy::evaluate_with_context(
                &request,
                &policy_context.request_context,
                &policy_context.config,
            )
            .context("failed to evaluate policy with Cedar engine")?;
            let matched_policies = if evaluation.explanation.matched_policy_ids.is_empty() {
                "none".to_owned()
            } else {
                evaluation.explanation.matched_policy_ids.join(",")
            };
            let (decision, approval_required, reason) = match &evaluation.decision {
                PolicyDecision::Allow => ("allow", false, evaluation.explanation.reason.as_str()),
                PolicyDecision::DenyByDefault { reason } => {
                    ("deny_by_default", true, reason.as_str())
                }
            };
            let diagnostics =
                palyra_policy::policy_explain_diagnostics_value(&request, &evaluation);
            if output::preferred_json(json) {
                return output::print_json_pretty(
                    &json!({
                        "decision": decision,
                        "principal": request.principal,
                        "action": request.action,
                        "resource": request.resource,
                        "approval_required": approval_required,
                        "reason": reason,
                        "matched_policies": evaluation.explanation.matched_policy_ids,
                        "diagnostics": diagnostics,
                        "policy_config": {
                            "source": policy_context.config_source,
                            "allowlisted_tools": policy_context.config.allowlisted_tools,
                        },
                        "explanation": {
                            "evaluated_with_cedar": evaluation.explanation.evaluated_with_cedar,
                            "diagnostics_errors": evaluation.explanation.diagnostics_errors,
                            "is_sensitive_action": evaluation.explanation.is_sensitive_action,
                            "is_allowlisted_tool": evaluation.explanation.is_allowlisted_tool,
                            "is_allowlisted_skill": evaluation.explanation.is_allowlisted_skill,
                            "is_tool_execute_principal_allowed": evaluation
                                .explanation
                                .is_tool_execute_principal_allowed,
                            "is_tool_execute_channel_allowed": evaluation
                                .explanation
                                .is_tool_execute_channel_allowed,
                            "requested_tool": evaluation.explanation.requested_tool,
                            "requested_skill": evaluation.explanation.requested_skill,
                            "request_capabilities": evaluation.explanation.request_capabilities,
                            "constructed_entities": evaluation.explanation.constructed_entities,
                        },
                    }),
                    "failed to encode policy explain output as JSON",
                );
            }
            if output::preferred_ndjson(json, false) {
                output::print_json_line(
                    &json!({
                        "decision": decision,
                        "principal": request.principal,
                        "action": request.action,
                        "resource": request.resource,
                        "approval_required": approval_required,
                        "reason": reason,
                        "matched_policies": evaluation.explanation.matched_policy_ids,
                        "policy_config_source": policy_context.config_source,
                    }),
                    "failed to encode policy explain output as NDJSON",
                )?;
                return std::io::stdout().flush().context("stdout flush failed");
            }
            match evaluation.decision {
                PolicyDecision::Allow => {
                    println!(
                        "decision=allow principal={} action={} resource={} reason={} matched_policies={} policy_config_source={}",
                        request.principal,
                        request.action,
                        request.resource,
                        evaluation.explanation.reason,
                        matched_policies,
                        policy_context.config_source,
                    );
                }
                PolicyDecision::DenyByDefault { reason } => {
                    println!(
                        "decision=deny_by_default principal={} action={} resource={} approval_required=true reason={} matched_policies={} policy_config_source={}",
                        request.principal,
                        request.action,
                        request.resource,
                        reason,
                        matched_policies,
                        policy_context.config_source,
                    );
                }
            }
            std::io::stdout().flush().context("stdout flush failed")
        }
    }
}

struct PolicyExplainContext {
    config: PolicyEvaluationConfig,
    request_context: palyra_policy::PolicyRequestContext,
    config_source: String,
}

fn load_policy_explain_context() -> Result<PolicyExplainContext> {
    let (allowlisted_tools, config_source) = load_policy_allowlisted_tools()?;
    Ok(PolicyExplainContext {
        config: PolicyEvaluationConfig { allowlisted_tools, ..PolicyEvaluationConfig::default() },
        request_context: palyra_policy::PolicyRequestContext::default(),
        config_source,
    })
}

fn load_policy_allowlisted_tools() -> Result<(Vec<String>, String)> {
    if let Ok(raw) = env::var("PALYRA_TOOL_CALL_ALLOWED_TOOLS") {
        return Ok((
            parse_policy_tool_allowlist(raw.as_str()),
            "env:PALYRA_TOOL_CALL_ALLOWED_TOOLS".to_owned(),
        ));
    }

    let Some(config_path) = resolve_policy_config_path()? else {
        return Ok((Vec::new(), "default:empty".to_owned()));
    };
    let raw = fs::read_to_string(config_path.as_path())
        .with_context(|| format!("failed to read policy config {}", config_path.display()))?;
    let document = toml::from_str::<toml::Value>(raw.as_str())
        .with_context(|| format!("failed to parse policy config {}", config_path.display()))?;
    Ok((
        read_policy_tool_allowlist_from_document(&document),
        format!("config:{}", config_path.display()),
    ))
}

fn resolve_policy_config_path() -> Result<Option<PathBuf>> {
    if let Some(path) = app::current_root_context()
        .as_ref()
        .and_then(|context| context.config_path().map(Path::to_path_buf))
        .filter(|path| path.exists())
    {
        return Ok(Some(path));
    }
    if let Ok(raw) = env::var("PALYRA_CONFIG") {
        if let Some(raw) = normalize_optional_text(raw.as_str()) {
            let path =
                parse_config_path(raw).with_context(|| "PALYRA_CONFIG contains an invalid path")?;
            return Ok(path.exists().then_some(path));
        }
    }
    Ok(default_config_search_paths().into_iter().find(|candidate| candidate.exists()))
}

fn read_policy_tool_allowlist_from_document(document: &toml::Value) -> Vec<String> {
    let Some(value) =
        document.get("tool_call").and_then(|tool_call| tool_call.get("allowed_tools"))
    else {
        return Vec::new();
    };
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .flat_map(|value| parse_policy_tool_allowlist(value))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_policy_tool_allowlist(raw: &str) -> Vec<String> {
    raw.split(',').map(str::trim).filter(|value| !value.is_empty()).map(ToOwned::to_owned).collect()
}
