# Web Console Runtime

This package now hosts the M34 A2UI renderer baseline for the upcoming web console.

## What ships in M34

- React + TypeScript web runtime (`Vite` build surface).
- Safe A2UI renderer with a bounded component set:
  - `text`
  - `markdown` (sanitized; no raw HTML injection)
  - `list`
  - `table`
  - `form` (limited controls)
  - `chart` (bar chart)
- Incremental patch processing queue with explicit frame budgets.
- Security tests:
  - XSS regression suite
  - Property/fuzz coverage for malformed patch payloads
  - Snapshot tests for deterministic rendering

## Local commands

- Install dependencies:
  - `npm --prefix apps/web ci`
- Lint:
  - `npm --prefix apps/web run lint`
- Typecheck:
  - `npm --prefix apps/web run typecheck`
- Tests:
  - `npm --prefix apps/web run test:run`
- Build:
  - `npm --prefix apps/web run build`

## Notes

- Renderer is fail-closed for malformed A2UI payloads and patch operations.
- Runtime avoids `dangerouslySetInnerHTML`; markdown rendering is tokenized into React elements.
