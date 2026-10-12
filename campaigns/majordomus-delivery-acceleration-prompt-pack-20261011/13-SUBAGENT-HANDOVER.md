# MULTI-AGENT / SESSION HANDOVER CONTRACT

Use only if actual mesh/subagent capability and safety isolation are available. Do not pretend delegation occurred.

## Worker assignment template
- Mission and canonical intent criterion IDs (resolved, pinned)
- Scope/path ownership and protected/shared generated files
- Base SHA, applicable policies and existing PR dependencies
- Exact expected change and anti-goals
- Positive and negative tests; mutation plan
- Deadline/SLO intent (not promise of async delivery)
- Read-only vs write permissions, effect class, remote mutation restrictions
- Artifact and evidence destination from canonical project model
- Completion means: code verified / PR integrated / release deployed (separate)

## Per-worker report
`worker-id; SHA; bound criteria; owned paths; changed files; tests with exact commands/results; before/after timings; derived outputs; PR URL; merge/release/deploy state; blockers; conflicting claims; next dependency; confidence`.

## Coordinator rules
- One owner per overlapping write set; no two generators on same artifact without fencing.
- Merge reports using canonical event/evidence model, not freeform chat memory.
- Reconcile worker results against git, test artifacts and task versions before acknowledging completion.
- Escalate a real blocker and continue unrelated tasks; avoid infinite retry or broad unattended branch spawning.
