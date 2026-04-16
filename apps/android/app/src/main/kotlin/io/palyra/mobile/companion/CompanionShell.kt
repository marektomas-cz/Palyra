package io.palyra.mobile.companion

enum class MobileSurface {
    APPROVALS,
    INBOX,
    SESSIONS,
    VOICE_NOTE,
    SETTINGS,
}

data class CompanionShellState(
    val locale: String = "en",
    val surface: MobileSurface = MobileSurface.INBOX,
    val contract: MobileBootstrapContract = MobileBootstrapContract(),
    val cachedState: MobileCachedState = MobileCachedState(),
    val recentSessions: List<MobileSessionRecap> = emptyList(),
    val approvals: List<MobileApprovalSummary> = emptyList(),
    val offlineBannerVisible: Boolean = false,
    val revokeBannerVisible: Boolean = false,
)

sealed interface CompanionAction {
    data class SwitchSurface(val surface: MobileSurface) : CompanionAction
    data class SetLocale(val locale: String) : CompanionAction
    data class SetOfflineBanner(val visible: Boolean) : CompanionAction
    data class SetRevokeBanner(val visible: Boolean) : CompanionAction
    data class ReplaceApprovals(val approvals: List<MobileApprovalSummary>) : CompanionAction
    data class ReplaceRecentSessions(val recaps: List<MobileSessionRecap>) : CompanionAction
}

fun reduceCompanionShell(
    state: CompanionShellState,
    action: CompanionAction,
): CompanionShellState {
    return when (action) {
        is CompanionAction.SwitchSurface -> state.copy(surface = action.surface)
        is CompanionAction.SetLocale -> state.copy(locale = action.locale)
        is CompanionAction.SetOfflineBanner -> state.copy(offlineBannerVisible = action.visible)
        is CompanionAction.SetRevokeBanner -> state.copy(revokeBannerVisible = action.visible)
        is CompanionAction.ReplaceApprovals -> state.copy(approvals = action.approvals)
        is CompanionAction.ReplaceRecentSessions -> state.copy(recentSessions = action.recaps)
    }
}
