# Web UI Architecture Milestone Plan

Status: approved closure-debt implementation plan for the `M35` and `M38` exit criteria in `roadmap/phase_0-7_fixed.md`.

## Why this exists

The current web console is functionally valid, but the composition boundary is too coarse for the next roadmap window. `apps/web/src/App.tsx` still owns auth, section routing, approvals, cron, channels, diagnostics, memory, skills, browser, audit, and shared presentation helpers in one root component. `apps/web/src/chat/ChatConsolePanel.tsx` already delivers real operator value, but it still combines session management, stream orchestration, transcript rendering, run diagnostics, and A2UI/canvas rendering inside one file.

This milestone exists to pay down that closure debt before new UI capabilities are layered on top.

## Current pressure points

- `apps/web/src/App.tsx`
  - owns session boot/login, theme state, section switching, and almost every operator surface
  - embeds Discord onboarding and channel-router flows directly in the root component
  - mixes API loading state, data shaping, and view rendering for unrelated capability domains
- `apps/web/src/chat/ChatConsolePanel.tsx`
  - owns chat sessions, composer, stream lifecycle, transcript state, run drawer state, and A2UI presentation
  - duplicates sensitive-value formatting concerns that should stay centralized
- `apps/web/src/consoleApi.ts`
  - already provides the right typed contract surface, but it is still one monolithic client rather than domain-grouped capability clients

## Architectural outcome

The UI architecture milestone should leave the console with thin composition shells and domain-owned feature modules:

- a root app shell that handles only boot, auth session state, theme, and top-level navigation
- section boundaries aligned to product domains such as approvals, cron, channels, diagnostics, memory, skills, browser, audit, and chat
- a shared typed data-access layer grouped by capability domain instead of one giant call surface
- centralized redaction, JSON formatting, and sensitive-value reveal controls shared across the console
- a modular chat workspace split into session list, transcript, composer, stream controller, run diagnostics drawer, and A2UI/canvas presentation units

## Delivery slices

### Slice 1: shared platform layer

- Create a shared console utility layer for:
  - sensitive-value redaction
  - JSON pretty-print rendering
  - common loading/error state helpers where reuse is real
- Reshape `apps/web/src/consoleApi.ts` into a domain-oriented client surface while preserving current HTTP contracts and test coverage.
- Keep the existing `ConsoleApiClient` entry point as a thin facade during migration so the refactor can land in small commits.

### Slice 2: root shell decomposition

- Reduce `apps/web/src/App.tsx` to:
  - app bootstrap
  - auth/session gate
  - theme persistence
  - top-level route or section selection
  - feature composition
- Move domain screens into dedicated modules by product boundary.
- Preserve same-origin, cookie-session, and CSRF behavior without introducing client-side auth bypasses.

### Slice 3: chat workspace decomposition

- Split `apps/web/src/chat/ChatConsolePanel.tsx` into:
  - session list and session actions
  - composer
  - transcript renderer
  - stream controller hook
  - run details drawer
  - A2UI/canvas presentation helpers
- Keep stream cancellation, run drawer refresh ordering, and approval-safe rendering behavior identical to the current console.

### Slice 4: guardrails

- Add an explicit UI structure guardrail so this debt does not immediately regress.
- Preferred guardrail:
  - a repository-local size check or lint rule that enforces file-size budgets on new or refactored UI roots
- Minimum acceptance:
  - `App.tsx` becomes a thin shell
  - `ChatConsolePanel.tsx` becomes a composition layer, not the implementation home for every chat concern

## Proposed file map

Exact filenames may change, but the responsibility split should be preserved:

- `apps/web/src/app/*`
  - app shell, session gate, section routing, shared layout
- `apps/web/src/features/approvals/*`
- `apps/web/src/features/cron/*`
- `apps/web/src/features/channels/*`
- `apps/web/src/features/diagnostics/*`
- `apps/web/src/features/memory/*`
- `apps/web/src/features/skills/*`
- `apps/web/src/features/browser/*`
- `apps/web/src/features/audit/*`
- `apps/web/src/features/chat/*`
- `apps/web/src/shared/redaction.ts`
- `apps/web/src/shared/json.ts`
- `apps/web/src/consoleApi/*`
  - domain-grouped clients behind a stable top-level facade

## Security and product constraints

- Keep sensitive-value handling centralized and redacted by default.
- Do not duplicate credential, token, cookie, or diagnostics redaction logic per feature module.
- Do not move business logic into untyped view helpers when the existing typed API layer can own the contract.
- Preserve the web console as the canonical full operator surface.
- Preserve existing tests around auth gating, approvals, diagnostics redaction, and chat streaming flows while refactoring.

## Acceptance criteria

This milestone is complete when all of the following are true:

- Existing operator behavior is preserved.
- New console sections can be added without editing a giant root component.
- `App.tsx` no longer owns unrelated domain workflows directly.
- `ChatConsolePanel.tsx` no longer owns every chat concern directly.
- Typed data access is grouped by capability domain.
- Shared redaction/JSON helpers exist in one place and are reused.
- A file-size or structure guardrail exists to prevent immediate regression.

## Validation plan

- `npm --prefix apps/web run lint`
- `npm --prefix apps/web run typecheck`
- `npm --prefix apps/web run test:run`
- `npm --prefix apps/web run build`
- Focused regression coverage for:
  - auth/session boot and sign-out
  - approvals workflows
  - diagnostics redaction
  - chat session lifecycle and streaming
  - A2UI rendering and patch safety

## Scope boundaries

- This milestone does not add new operator capabilities by itself.
- This milestone does not replace `/console/v1` contracts.
- This milestone does not move provider credential UX out of the canonical dashboard flows.
