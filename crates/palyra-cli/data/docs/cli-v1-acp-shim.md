# CLI v1 + ACP Bridge

Purpose: capture the current CLI v1 surface and the ACP bridge entry points used for IDE-style
agent integration and release-ready operator workflows.

## Top-level command families

- `palyra acp ...`
- `palyra agent ...`
- `palyra agents ...`
- `palyra approvals ...`
- `palyra auth ...`
- `palyra browser ...`
- `palyra channels ...`
- `palyra completion ...`
- `palyra config ...`
- `palyra configure ...`
- `palyra cron ...`
- `palyra docs ...`
- `palyra gateway ...`
- `palyra memory ...`
- `palyra node ...`
- `palyra nodes ...`
- `palyra onboarding wizard ...`
- `palyra patch apply ...`
- `palyra secrets ...`
- `palyra sessions ...`
- `palyra setup ...`
- `palyra skills ...`
- `palyra support-bundle ...`
- `palyra tunnel ...`
- `palyra update ...`
- `palyra uninstall ...`

Compatibility aliases remain available where implemented, including `init`, `daemon`, and
`onboard`.

## ACP bridge posture

- `palyra acp ...` is the preferred ACP bridge family.
- ACP commands stay discoverable in CLI help and operator docs so IDE integrations can target a
  stable bridge surface.
- ACP bridge behavior follows the same auth, routing, and policy posture as the rest of the CLI
  gateway-facing surfaces.

## Release-ready operator guidance

- Prefer `palyra setup` over `palyra init` for bootstrap and packaged install examples.
- Prefer `palyra gateway` over `palyra daemon` for runtime and admin examples.
- Prefer `palyra onboarding wizard` over `palyra onboard` when clarity matters in docs and smoke
  coverage.
- Keep `palyra docs` available in portable installs so the ACP bridge, migration notes, and release
  checklist remain available offline.

## Browser and service lifecycle notes

- `palyra browser` is a real operator surface over `palyra-browserd`, not a pure lifecycle stub.
- `palyra node` and `palyra nodes` remain packaged and covered by release smoke because service and
  fleet workflows are part of the shipped operator contract.
- `palyra update --archive <zip> --dry-run` and `palyra uninstall --dry-run` are part of the
  package lifecycle contract and stay discoverable in help and docs.
