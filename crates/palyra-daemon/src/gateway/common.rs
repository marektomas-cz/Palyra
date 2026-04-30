use super::*;

pub(crate) fn map_orchestrator_store_error(operation: &str, error: JournalError) -> Status {
    match error {
        JournalError::DuplicateRunId { run_id } => {
            Status::already_exists(format!("orchestrator run already exists: {run_id}"))
        }
        JournalError::DuplicateTapeSequence { run_id, seq } => Status::already_exists(format!(
            "orchestrator tape already contains seq={seq} for run {run_id}"
        )),
        JournalError::RunNotFound { run_id } => {
            Status::not_found(format!("orchestrator run not found: {run_id}"))
        }
        JournalError::PayloadTooLarge { payload_kind, actual_bytes, max_bytes } => {
            Status::invalid_argument(format!(
                "{payload_kind} payload exceeds maximum size ({actual_bytes} > {max_bytes})"
            ))
        }
        JournalError::SessionIdentityMismatch { session_id } => Status::failed_precondition(
            format!("orchestrator session identity mismatch for session: {session_id}"),
        ),
        JournalError::SessionNotFound { selector } => {
            Status::not_found(format!("orchestrator session not found for selector: {selector}"))
        }
        JournalError::InvalidSessionSelector { reason } => {
            Status::invalid_argument(format!("invalid orchestrator session selector: {reason}"))
        }
        JournalError::FlowNotFound { flow_id } => {
            Status::not_found(format!("flow not found: {flow_id}"))
        }
        JournalError::FlowStepNotFound { flow_id, step_id } => {
            Status::not_found(format!("flow step not found: {flow_id}/{step_id}"))
        }
        JournalError::FlowRevisionConflict {
            flow_id,
            expected_revision,
            actual_revision,
        } => Status::aborted(format!(
            "flow revision conflict for {flow_id}: expected {expected_revision}, found {actual_revision}"
        )),
        JournalError::InvalidArgument(message) => Status::invalid_argument(message),
        JournalError::DuplicateToolResultArtifactId { artifact_id } => {
            Status::already_exists(format!("tool result artifact already exists: {artifact_id}"))
        }
        JournalError::ToolResultArtifactNotFound { artifact_id } => {
            Status::not_found(format!("tool result artifact not found: {artifact_id}"))
        }
        JournalError::ToolResultArtifactDigestMismatch { artifact_id } => {
            Status::failed_precondition(format!(
                "tool result artifact digest mismatch: {artifact_id}"
            ))
        }
        JournalError::ToolResultArtifactScopeMismatch { artifact_id } => {
            Status::permission_denied(format!(
                "tool result artifact is outside the current run/session scope: {artifact_id}"
            ))
        }
        JournalError::ToolResultArtifactReadDenied { artifact_id, reason } => {
            Status::permission_denied(format!(
                "tool result artifact read denied for {artifact_id}: {reason}"
            ))
        }
        other => Status::internal(format!("{operation} failed: {other}")),
    }
}

pub(crate) fn map_agent_registry_error(operation: &str, error: AgentRegistryError) -> Status {
    match error {
        AgentRegistryError::AgentNotFound(agent_id) => {
            Status::not_found(format!("agent not found: {agent_id}"))
        }
        AgentRegistryError::DuplicateAgentId(agent_id) => {
            Status::already_exists(format!("agent already exists: {agent_id}"))
        }
        AgentRegistryError::AgentDirCollision(agent_id) => Status::already_exists(format!(
            "agent directory overlaps with existing agent {agent_id}"
        )),
        AgentRegistryError::WorkspaceRootEscape(path)
        | AgentRegistryError::DuplicateWorkspaceRoot(path)
        | AgentRegistryError::InvalidSessionId(path) => Status::invalid_argument(path),
        AgentRegistryError::DefaultAgentNotConfigured => {
            Status::failed_precondition("default agent is not configured")
        }
        AgentRegistryError::InvalidPath { field, message } => {
            Status::invalid_argument(format!("{field}: {message}"))
        }
        AgentRegistryError::RegistryLimitExceeded => {
            Status::resource_exhausted("agent registry limits exceeded")
        }
        other => Status::internal(format!("{operation} failed: {other}")),
    }
}

pub(crate) fn map_cron_store_error(operation: &str, error: JournalError) -> Status {
    match error {
        JournalError::CronJobNotFound { job_id } => {
            Status::not_found(format!("cron job not found: {job_id}"))
        }
        JournalError::CronJobHasActiveRuns { job_id } => {
            Status::failed_precondition(format!("cron job has an active run: {job_id}"))
        }
        JournalError::CronRunNotFound { run_id } => {
            Status::not_found(format!("cron run not found: {run_id}"))
        }
        JournalError::DuplicateCronJobId { job_id } => {
            Status::already_exists(format!("cron job already exists: {job_id}"))
        }
        JournalError::DuplicateCronRunId { run_id } => {
            Status::already_exists(format!("cron run already exists: {run_id}"))
        }
        JournalError::PayloadTooLarge { payload_kind, actual_bytes, max_bytes } => {
            Status::invalid_argument(format!(
                "{payload_kind} payload exceeds maximum size ({actual_bytes} > {max_bytes})"
            ))
        }
        other => Status::internal(format!("{operation} failed: {other}")),
    }
}

pub(crate) fn map_approval_store_error(operation: &str, error: JournalError) -> Status {
    match error {
        JournalError::ApprovalNotFound { approval_id } => {
            Status::not_found(format!("approval record not found: {approval_id}"))
        }
        JournalError::DuplicateApprovalId { approval_id } => {
            Status::already_exists(format!("approval record already exists: {approval_id}"))
        }
        JournalError::PayloadTooLarge { payload_kind, actual_bytes, max_bytes } => {
            Status::invalid_argument(format!(
                "{payload_kind} payload exceeds maximum size ({actual_bytes} > {max_bytes})"
            ))
        }
        other => Status::internal(format!("{operation} failed: {other}")),
    }
}

pub(crate) fn map_memory_store_error(operation: &str, error: JournalError) -> Status {
    match error {
        JournalError::MemoryNotFound { memory_id } => {
            Status::not_found(format!("memory item not found: {memory_id}"))
        }
        JournalError::DuplicateMemoryId { memory_id } => {
            Status::already_exists(format!("memory item already exists: {memory_id}"))
        }
        JournalError::DuplicateRecallArtifactId { artifact_id } => {
            Status::already_exists(format!("recall artifact already exists: {artifact_id}"))
        }
        JournalError::DuplicateWorkspacePath { path } => {
            Status::already_exists(format!("workspace document already exists for path: {path}"))
        }
        JournalError::WorkspaceDocumentNotFound { path } => {
            Status::not_found(format!("workspace document not found for path: {path}"))
        }
        JournalError::InvalidWorkspacePath { reason } => {
            Status::invalid_argument(format!("invalid workspace path: {reason}"))
        }
        JournalError::InvalidWorkspaceContent { reason } => {
            Status::invalid_argument(format!("invalid workspace content: {reason}"))
        }
        JournalError::PayloadTooLarge { payload_kind, actual_bytes, max_bytes } => {
            Status::invalid_argument(format!(
                "{payload_kind} payload exceeds maximum size ({actual_bytes} > {max_bytes})"
            ))
        }
        other => Status::internal(format!("{operation} failed: {other}")),
    }
}

pub(crate) fn map_skill_store_error(operation: &str, error: JournalError) -> Status {
    match error {
        JournalError::PayloadTooLarge { payload_kind, actual_bytes, max_bytes } => {
            Status::invalid_argument(format!(
                "{payload_kind} payload exceeds maximum size ({actual_bytes} > {max_bytes})"
            ))
        }
        other => Status::internal(format!("{operation} failed: {other}")),
    }
}

pub(crate) fn map_canvas_store_error(operation: &str, error: JournalError) -> Status {
    match error {
        JournalError::DuplicateCanvasStateVersion { canvas_id, state_version } => {
            Status::already_exists(format!(
                "canvas state already exists for canvas {canvas_id} at version {state_version}"
            ))
        }
        JournalError::CanvasStateNotFound { canvas_id } => {
            Status::not_found(format!("canvas state not found: {canvas_id}"))
        }
        JournalError::InvalidCanvasReplay { canvas_id, reason } => Status::failed_precondition(
            format!("invalid canvas replay state for {canvas_id}: {reason}"),
        ),
        JournalError::PayloadTooLarge { payload_kind, actual_bytes, max_bytes } => {
            Status::invalid_argument(format!(
                "{payload_kind} payload exceeds maximum size ({actual_bytes} > {max_bytes})"
            ))
        }
        other => Status::internal(format!("{operation} failed: {other}")),
    }
}

pub(crate) fn current_unix_ms_status() -> Result<i64, Status> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Status::internal(format!("system time before unix epoch: {error}")))?;
    Ok(elapsed.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::map_memory_store_error;
    use crate::journal::JournalError;
    use tonic::Code;

    #[test]
    fn map_memory_store_error_maps_invalid_workspace_path_to_invalid_argument() {
        let status = map_memory_store_error(
            "upsert workspace document",
            JournalError::InvalidWorkspacePath { reason: "absolute paths are not allowed".into() },
        );

        assert_eq!(status.code(), Code::InvalidArgument);
        assert!(status.message().contains("invalid workspace path"));
    }

    #[test]
    fn map_memory_store_error_maps_workspace_document_not_found_to_not_found() {
        let status = map_memory_store_error(
            "get workspace document",
            JournalError::WorkspaceDocumentNotFound { path: "docs/missing.md".into() },
        );

        assert_eq!(status.code(), Code::NotFound);
        assert!(status.message().contains("docs/missing.md"));
    }

    #[test]
    fn map_memory_store_error_maps_duplicate_workspace_path_to_already_exists() {
        let status = map_memory_store_error(
            "upsert workspace document",
            JournalError::DuplicateWorkspacePath { path: "docs/guide.md".into() },
        );

        assert_eq!(status.code(), Code::AlreadyExists);
        assert!(status.message().contains("docs/guide.md"));
    }
}
