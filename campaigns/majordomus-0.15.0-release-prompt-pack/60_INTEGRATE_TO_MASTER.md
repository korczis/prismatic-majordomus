# Milestone 6 — Integrate the proven candidate to master

Preconditions: candidate contains current master, readiness green, candidate pushed, binding CI green, required intent/plan evidence recorded, integration policy allows it.

1. Re-read master immediately before integration.
2. If moved, refresh candidate and rerun impacted/binding checks.
3. Use current PR-integration control plane.
4. No admin bypass, forbidden force-push, stale-head merge, unsafe override, or red binding CI.
5. Confirm intended commits become reachable from master.
6. Confirm no member branch contains required unique work left behind.
7. Observe binding master CI.
8. Repair merge-caused failures immediately through canonical workflow.

Acceptance: release functionality on current master, integration commit identified, binding master CI green, plan/intent state reflects landed work, no release-critical member stranded.

Report PR, master before/after, integration method, CI run, member status.
