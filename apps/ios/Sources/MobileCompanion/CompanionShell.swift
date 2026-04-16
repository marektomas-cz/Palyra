import Foundation

enum MobileSurface: String, Codable {
    case approvals
    case inbox
    case sessions
    case voiceNote = "voice_note"
    case settings
}

struct CompanionShellState: Codable, Equatable {
    var locale: String = "en"
    var surface: MobileSurface = .inbox
    var contract: MobileBootstrapContract = .init()
    var cachedState: MobileCachedState = .init()
    var recentSessions: [MobileSessionRecap] = []
    var approvals: [MobileApprovalSummary] = []
    var offlineBannerVisible: Bool = false
    var revokeBannerVisible: Bool = false
}

enum CompanionAction: Equatable {
    case switchSurface(MobileSurface)
    case setLocale(String)
    case setOfflineBanner(Bool)
    case setRevokeBanner(Bool)
    case replaceApprovals([MobileApprovalSummary])
    case replaceRecentSessions([MobileSessionRecap])
}

func reduceCompanionShell(
    state: CompanionShellState,
    action: CompanionAction
) -> CompanionShellState {
    var next = state
    switch action {
    case let .switchSurface(surface):
        next.surface = surface
    case let .setLocale(locale):
        next.locale = locale
    case let .setOfflineBanner(visible):
        next.offlineBannerVisible = visible
    case let .setRevokeBanner(visible):
        next.revokeBannerVisible = visible
    case let .replaceApprovals(approvals):
        next.approvals = approvals
    case let .replaceRecentSessions(recaps):
        next.recentSessions = recaps
    }
    return next
}
