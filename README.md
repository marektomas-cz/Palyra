# Palyra

This repository is currently in the development phase.

![This is Fine](https://user-images.githubusercontent.com/93007558/216892041-5599d3d8-50e0-4d46-8171-5021d69d7745.gif)

## Runtime surfaces

- `palyrad`: daemon runtime with admin HTTP, gateway gRPC, orchestrator run loop, and durable SQLite journal.
- `palyra`: operator CLI for status checks, policy/config helpers, orchestrator runs, and scheduler operations.

## Scheduler v1 (M16)

Scheduler v1 adds a durable cron subsystem backed by SQLite:

- schedule types: `cron`, `every`, `at`
- job controls: create, update, delete, enable/disable, run-now
- execution controls: concurrency (`forbid`, `replace`, `queue(1)`), retries, jitter, misfire policy
- run history: status, redacted errors, token and tool-usage counters

The daemon executes scheduled jobs as standard orchestrator runs (`ResolveSession` + `RunStream`) with policy enforcement intact. Sensitive tools still require explicit approval and remain denied when no interactive approval path is available.

### CLI quickstart

```bash
# create a recurring job
palyra cron add \
  --name "Health summary" \
  --prompt "Summarize daemon health" \
  --schedule-type every \
  --every-interval "15m" \
  --owner "user:ops" \
  --channel "system:cron"

# list jobs and inspect details
palyra cron list
palyra cron show --job-id <job_ulid>

# trigger and inspect run logs
palyra cron run-now --job-id <job_ulid>
palyra cron logs --job-id <job_ulid>
```
