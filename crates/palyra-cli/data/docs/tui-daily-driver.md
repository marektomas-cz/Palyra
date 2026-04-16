# TUI Daily-Driver Workflows

Purpose: capture the terminal-first workflows that make the Palyra TUI viable for daily work
without depending on the web console for routine compose, inspect, and recover loops.

## Multiline composer

- `Enter` sends the current prompt.
- `Alt+Enter` and `Ctrl+J` insert a newline without sending.
- Arrow keys, `Home`, and `End` move across the full draft, including multiline content.
- `Ctrl+A` selects the full draft.
- `Ctrl+U` clears the draft and pending attachments together.
- Drafts are cached per session so switching away and coming back restores in-progress work.

## TUI attachments

- `/attach <path>` uploads a local file into the active chat session.
- `/attach list` shows the pending queue.
- `/attach remove <index|id|name>` drops a single pending attachment.
- `/attach clear` clears the queue.
- `Ctrl+O` seeds `/attach ` in the composer for fast path entry.

Uploaded attachments surface filename, type, size, and any derived artifact metadata in the TUI
and participate in the same context-budget accounting shown by the status surfaces.

## Status bar and recap

- `/status` returns a compact runtime summary for the active session.
- `/status detail` adds context-budget, workspace, and background-task detail.
- The composer footer surfaces context fill, pending approvals, background work, and attachment
  state directly in the TUI.
- Resume flows show a recap banner with the latest summary, approvals, touched artifacts, and
  suggested next actions.

## Session family navigation

- `/resume parent`
- `/resume sibling`
- `/resume child`
- `/resume family`

The TUI session picker and slash palette use the same family metadata as the web and desktop
surfaces so branch relationships, relative titles, and recap cues stay aligned across surfaces.

## Workspace inspection and rollback

- `/workspace` lists the latest run artifacts, changed files, and checkpoint inventory.
- `/workspace changed` filters to changed items only.
- `/workspace show <index|artifact-id>` renders an artifact preview in the transcript.
- `/workspace open <index|artifact-id>` opens a local workspace file in the default OS handler when
  the artifact resolves inside a known workspace root.
- `/workspace handoff` prints a cross-surface handoff URL for the web workspace inspector.
- `/workspace handoff open` attempts to open that handoff in a browser immediately.

Rollback flows stay explicit and confirmation-gated:

- `/rollback` lists available checkpoints and the next inspection steps.
- `/rollback diff <checkpoint-id>` previews the delta for a checkpoint.
- `/rollback restore <checkpoint-id>` prints the restore summary and required confirmation.
- `/rollback restore <checkpoint-id> --confirm` restores into a safe branch session.
- `/rollback restore <checkpoint-id> --confirm --in-place` restores in the current session.

## Discoverability and tips

- `F8` toggles the lightweight TUI tip surface.
- `/help` and the slash palette provide context-aware examples for attachments, resume, workspace,
  and rollback flows.
- Shortcut hints remain visible in the footer and composer chrome instead of being hidden in source
  code only.
- Voice remains desktop-first. The TUI intentionally avoids advertising a partial push-to-talk path
  until it would be production-quality.
