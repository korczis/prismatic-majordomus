# Operation Zero: Autonomous Night Shift

Repository: `korczis/prismatic-majordomus`

## Goal

Drain open PRs to zero by safe integration, reconcile related issues/milestones/intents, issue a distinct policy-compliant version transaction for each **independent completion unit**, release/deploy/verify, and repeat without operator intervention. Do not manufacture releases for administrative closures or duplicate cascaded events.

## Run

1. Open Claude Code (Opus/Fable) at the repository root with the normal repository credentials, tool permissions, and long-running session supervisor already authorized by the operator.
2. Paste the complete content of `00-MASTER-ORCHESTRATOR.md` as the **one entry prompt**.
3. The orchestrator reads `01` through `09` as specialized execution contracts and executes them in dependency order. If files aren't in its workspace, paste `00` and attach this archive or copy these files into a safe temporary working directory.
4. Allow the existing repo's CI, branch protections, authentication and release policies to govern operations. Never bypass them.

## Important

A prompt alone is not a durable background daemon. The actor must remain alive under an existing approved supervisor, runner or other persistent execution environment; on restart it must resume from persisted journal. No unattended runner has been installed by this pack.

### Expected result

- Real GitHub PR count becomes zero if all PRs are safely resolvable.
- All distinct eligible completion units have exactly one version allocation and corresponding release proof.
- No staged release obligation is lost when a process dies.
- Any unresolvable external blockers are reported truthfully; remaining independent work continues.

## Files

- `00-MASTER-ORCHESTRATOR.md` entry prompt
- `01-RECONNAISSANCE.md` canonical inventory and dependency DAG
- `02-SCHEDULER.md` bounded concurrency and prioritization
- `03-INTEGRATION.md` safe per-PR merge protocol
- `04-COMPLETION-RELEASE.md` exactly-once version/release/deploy semantics
- `05-RECOVERY.md` crash-consistent journal, idempotency and watchdog
- `06-PROJECTIONS.md` CLI/MCP/UI derived live progress
- `07-TEST-GATES.md` proof and failure-injection matrix
- `08-FINAL-RECONCILIATION.md` zero-backlog acceptance
- `09-OPERATOR-REPORT.md` terse operational status and report
