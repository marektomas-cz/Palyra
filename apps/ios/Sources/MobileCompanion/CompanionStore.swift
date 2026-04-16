import Foundation

struct MobileVoiceNoteDraft: Codable, Equatable {
    var draftId: String
    var targetSessionId: String?
    var transcriptText: String
    var transcriptReviewed: Bool
    var durationMs: Int?
}

struct MobileCachedState: Codable, Equatable {
    var approvalsCacheKey: String = "mobile.approvals.cache"
    var sessionsCacheKey: String = "mobile.sessions.cache"
    var inboxCacheKey: String = "mobile.inbox.cache"
    var outboxQueueKey: String = "mobile.voice-note.outbox"
    var revokeMarkerKey: String = "mobile.auth.revoked"
    var revoked: Bool = false
    var sessionExpired: Bool = false
    var quietHoursEnabled: Bool = true
    var voiceNoteOutbox: [MobileVoiceNoteDraft] = []
}

protocol CompanionStore {
    func load() -> MobileCachedState
    func persist(_ state: MobileCachedState)
    func markRevoked()
    func clearRevokeMarker()
}
