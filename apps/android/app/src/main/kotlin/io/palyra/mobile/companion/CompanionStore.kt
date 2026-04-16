package io.palyra.mobile.companion

data class MobileVoiceNoteDraft(
    val draftId: String,
    val targetSessionId: String? = null,
    val transcriptText: String,
    val transcriptReviewed: Boolean,
    val durationMs: Long? = null,
)

data class MobileCachedState(
    val approvalsCacheKey: String = "mobile.approvals.cache",
    val sessionsCacheKey: String = "mobile.sessions.cache",
    val inboxCacheKey: String = "mobile.inbox.cache",
    val outboxQueueKey: String = "mobile.voice-note.outbox",
    val revokeMarkerKey: String = "mobile.auth.revoked",
    val revoked: Boolean = false,
    val sessionExpired: Boolean = false,
    val quietHoursEnabled: Boolean = true,
    val voiceNoteOutbox: List<MobileVoiceNoteDraft> = emptyList(),
)

interface CompanionStore {
    fun load(): MobileCachedState
    fun persist(state: MobileCachedState)
    fun markRevoked()
    fun clearRevokeMarker()
}
