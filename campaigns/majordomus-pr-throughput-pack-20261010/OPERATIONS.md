# Operations: bootstrap, drain, monitor, rollback

## Operator procedure
1. At live HEAD capture current master SHA, current open-PR listing and artifact copy of `majordomus prs refresh/status` structured output when actually supported; record unknowns.
2. Verify current `.ai/repo/ci/queue.yaml`, `docs/INTEGRATION.md`, ADR 0101 and branch protection. DO NOT enable auto merges by merely editing workflow permissions. Introduce explicit policy approval for active mode.
3. Read-only: explain root critical path and candidate dispositions, dry-run any proposed drain, validate simulated git merge driver in isolated checkout.
4. Execute current canonical quality gates on reviewed implementation PR, obtain required approval and merge legally.
5. Enable event triggers with least privilege and one writer, start with observe+prepare, then policy-authorized drain. Observe Github Actions summary after each batch. Stop when operator action needed.
6. Refresh after every merge; do not force rebase protected/shared work. Retarget and delete only proven safe artifacts.
7. Run canonical version analyze/bump; do not invent semantic version. Release/tag under policy, verify actual published assets/digest/provenance, Pages URL+served commit; roll out typed fleet using authorization, bounded changes and post-deploy health.
8. Monitor WIP, oldest actionable, p50/p95, job delays, failure rates and runtime convergence. Check partial fleet failures individually.

## Immediate safe rollback
- Pause acting queue triggers, retain read-only measurements.
- Abandon stale lease after fencing/recovery proof; never allow overlapping active writers.
- Revert offending implementation via normal protected-branch PR; no force push master.
- For failed rollout, run supported fleet/package previous-version rollback; validate readiness and mesh membership per machine.
- Preserve audit trail and exact SHA/version/evidence; do not erase dirty worktrees or hide partial deployment.

## Demonstration
In ephemeral test repo, prepare 3 independent small PRs + a two-deep stack + one true semantic conflict + one draft. Simulate duplicate events and master moving during integration. Run reconcile and safe drain under explicit test policy; prove eligible candidates merge once, descendants retarget, conflict stays blocked with owner/next action, dirty worktree preserved, CI proof exact, derived CLI/API/Cockpit identical. Then verify released sample binary version, docs published, and fleet staging before real rollout if provided.
