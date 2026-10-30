# Operator runbook

## Invocation

Copy `00-START-HERE.md` into a Claude Code session opened at a clean project worktree, and tell the agent to execute mission files in order while abiding by `.ai/` and owner authorization policies. Give parallel workers ONLY non-overlapping missions from the graph. This is an instruction pack, not a command with privilege to mutate GitHub by itself.

## Read-only preflight examples

```bash
pwd
git rev-parse --show-toplevel
git status --short --branch
git rev-parse HEAD
git remote -v
find . -maxdepth 3 -name AGENTS.md -o -name CLAUDE.md
# Then read repo canonical .ai policy, actual CLI help, and docs.
```

Never prescribe `cargo test --workspace` blindly: the workspace may be nested. Discover actual cargo manifest path and just tasks. `gh` commands require credentials and repository rights. The last audited master baseline is historical, not pinning instructions.

## Coordinating workers

- Coordinator owns baseline ledger, issue/PR DAG and integration lane.
- Worker A owns mesh journal and trust, scoped code paths only.
- Worker B owns HTTP security and MCP (coordinate overlaps in server types).
- Worker C owns CI/release scripts, not trunk writes until P0/branch protection proof.
- Worker D owns external adoption fixtures and UX tests.
- Reviewer owns independent failure injection and sign-off, no implementation conflict.

Use Majordomus native peer/claim/handover protocols if present. If no mesh, use Git/worktrees and explicit scoped handover. Do not pretend peer cooperation works simply because multiple terminals exist. Prefer PR-per-independent invariant. Avoid destructive `git clean -fdx` on shared worktrees.

## Reporting schema (conceptual, align with current repo canonical schema)

```json
{
  "repo": "korczis/prismatic-majordomus",
  "audited_head": "full SHA",
  "observed_at": "ISO-8601",
  "finding_id": "I2706",
  "priority": "P0",
  "disposition": "verified|in_progress|blocked|superseded|not_reproducible|unknown",
  "canonical_issue": "GitHub issue URL",
  "actual_code": ["path + function"],
  "tests": [{"command": "exact command", "sha": "full SHA", "result": "passed|failed|not_run", "artifact": "path or URL"}],
  "pr": "URL or null",
  "landed_commit": "full SHA or null",
  "release_evidence": "URL or null",
  "remaining_risk": "precise plain-language explanation"
}
```

## Stop conditions

Stop ONLY the unsafe operation, not all useful work, when: branch protection impossible to satisfy, credentials missing, public production mutation unauthorized, active worktree collision, tests uncover data loss, regression risk cannot be bounded. Continue diagnostic, test-only, and non-conflicting remediation. Escalate exact required decision. Never fabricate owner approval or bypass flags.
