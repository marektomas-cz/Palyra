use super::*;

impl ChannelPlatform {
    pub fn media_snapshot(&self) -> Result<Value, ChannelPlatformError> {
        serde_json::to_value(self.media_store.build_global_snapshot()?).map_err(|error| {
            ChannelPlatformError::InvalidInput(format!(
                "failed to serialize media diagnostics snapshot: {error}"
            ))
        })
    }

    pub fn store_console_chat_attachment(
        &self,
        request: ConsoleChatAttachmentStoreRequestView<'_>,
    ) -> Result<MediaArtifactPayload, ChannelPlatformError> {
        let attachment_id = Ulid::new().to_string();
        self.media_store
            .store_console_attachment(ConsoleAttachmentStoreRequest {
                connector_id: "console_chat",
                session_id: request.session_id,
                principal: request.principal,
                device_id: request.device_id,
                channel: request.channel,
                attachment_id: attachment_id.as_str(),
                filename: request.filename,
                declared_content_type: request.declared_content_type,
                bytes: request.bytes,
            })
            .map_err(ChannelPlatformError::from)
    }

    pub fn load_console_chat_attachment(
        &self,
        artifact_id: &str,
        session_id: &str,
        principal: &str,
        device_id: &str,
        channel: Option<&str>,
    ) -> Result<Option<MediaArtifactPayload>, ChannelPlatformError> {
        self.media_store
            .load_console_attachment(artifact_id, session_id, principal, device_id, channel)
            .map_err(ChannelPlatformError::from)
    }

    pub fn list_console_chat_attachments(
        &self,
        session_id: &str,
        principal: &str,
        device_id: &str,
        channel: Option<&str>,
    ) -> Result<Vec<MediaArtifactPayload>, ChannelPlatformError> {
        self.media_store
            .list_console_attachment_payloads(session_id, principal, device_id, channel)
            .map_err(ChannelPlatformError::from)
    }

    pub fn upsert_console_chat_derived_artifact(
        &self,
        request: MediaDerivedArtifactUpsertRequest<'_>,
    ) -> Result<MediaDerivedArtifactRecord, ChannelPlatformError> {
        self.media_store.upsert_derived_artifact(request).map_err(ChannelPlatformError::from)
    }

    pub fn upsert_console_chat_failed_derived_artifact(
        &self,
        request: MediaFailedDerivedArtifactUpsertRequest<'_>,
    ) -> Result<MediaDerivedArtifactRecord, ChannelPlatformError> {
        self.media_store.upsert_failed_derived_artifact(request).map_err(ChannelPlatformError::from)
    }

    pub fn list_console_chat_derived_artifacts(
        &self,
        session_id: &str,
        principal: &str,
        device_id: &str,
        channel: Option<&str>,
    ) -> Result<Vec<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .list_session_derived_artifacts(session_id, principal, device_id, channel)
            .map_err(ChannelPlatformError::from)
    }

    pub fn list_attachment_derived_artifacts(
        &self,
        source_artifact_id: &str,
    ) -> Result<Vec<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .list_attachment_derived_artifacts(source_artifact_id)
            .map_err(ChannelPlatformError::from)
    }

    pub fn get_derived_artifact(
        &self,
        derived_artifact_id: &str,
    ) -> Result<Option<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .get_derived_artifact(derived_artifact_id)
            .map_err(ChannelPlatformError::from)
    }

    pub fn list_linked_derived_artifacts(
        &self,
        workspace_document_id: Option<&str>,
        memory_item_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .list_linked_derived_artifacts(workspace_document_id, memory_item_id, limit)
            .map_err(ChannelPlatformError::from)
    }

    pub fn link_derived_artifact_targets(
        &self,
        derived_artifact_id: &str,
        workspace_document_id: Option<&str>,
        memory_item_id: Option<&str>,
    ) -> Result<(), ChannelPlatformError> {
        self.media_store
            .link_derived_artifact_targets(
                derived_artifact_id,
                workspace_document_id,
                memory_item_id,
            )
            .map_err(ChannelPlatformError::from)
    }

    pub fn select_console_chat_derived_chunks(
        &self,
        source_artifact_ids: &[String],
        query: &str,
        selection_budget_chars: Option<usize>,
    ) -> Result<Vec<MediaDerivedArtifactSelection>, ChannelPlatformError> {
        self.media_store
            .select_derived_prompt_chunks(source_artifact_ids, query, selection_budget_chars)
            .map_err(ChannelPlatformError::from)
    }

    pub fn quarantine_derived_artifact(
        &self,
        derived_artifact_id: &str,
        reason: Option<&str>,
    ) -> Result<Option<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .quarantine_derived_artifact(derived_artifact_id, reason)
            .map_err(ChannelPlatformError::from)
    }

    pub fn release_derived_artifact(
        &self,
        derived_artifact_id: &str,
    ) -> Result<Option<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .release_derived_artifact(derived_artifact_id)
            .map_err(ChannelPlatformError::from)
    }

    pub fn mark_derived_artifact_recompute_required(
        &self,
        derived_artifact_id: &str,
        required: bool,
    ) -> Result<Option<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .mark_derived_artifact_recompute_required(derived_artifact_id, required)
            .map_err(ChannelPlatformError::from)
    }

    pub fn purge_derived_artifact(
        &self,
        derived_artifact_id: &str,
    ) -> Result<Option<MediaDerivedArtifactRecord>, ChannelPlatformError> {
        self.media_store
            .purge_derived_artifact(derived_artifact_id)
            .map_err(ChannelPlatformError::from)
    }

    pub fn derived_stats(&self) -> Result<MediaDerivedStatsSnapshot, ChannelPlatformError> {
        self.media_store.derived_stats().map_err(ChannelPlatformError::from)
    }
}
