# Majordomus Autonomous Delivery Convergence · 2026-10-10

Goal: one authorized `majordomus prs drain --until-settled` launches a persistent intent/milestone-driven, mesh-capable delivery supervisor. It plans/replans, repairs, checks, integrates, releases and deploys where authorized, without manual repeated `proceed using majordomus mesh` prompts. Convergence can be SATISFIED or explicit BLOCKED; never invent success.

## Usage
Start in root of LIVE repo. Pass `prompts/00-live-audit.md` to Claude Code. Each prompt file is one proposed milestone; each `Issue` heading is a work item. Use existing `.ai/repo/project` issue/milestone model, rather than copying these files into a parallel management system. Progress 00→01→02→03→04→05→06 with dependency-aware overlap. Confirm authority through user-established local config for any unattended merge, release or deploy. Do NOT blindly paste examples of commands if CLI does not yet support them. Implement them.

## Priority / invariants
Preserve: typed canonical ownership, evidence freshness, GitHub required checks, signed provenance, secret safety, trust != authority, safe concurrent writes, idempotency and auditability.
Relax/change: exact initial scope as immutable obstacle (replace with controlled amend), conversational continuation, blanket serialized task execution, repetition of full derive or every check, human acknowledgement for routine authorized actions, excessive repeated review of unchanged decisions.

## Proposed UX (not verified existing)
```
majordomus prs drain --until-settled --autonomous --detach
majordomus prs runs
majordomus prs run status <id>
majordomus prs run follow <id>
majordomus prs run stop <id>
majordomus prs run resume <id>
```

## Architectural caution
Recent public docs show `prs drain --continuous`, but mesh discovery is observation, peer board is volatile and neither implies task execution. Build on actual available modules, not a second control plane.

## Success measure
Real unattended sample on both self-hosted Majordomus and clean consumer, with bounded authority, real checks and verified delivery. Measure p50/p95 elapsed time, PR backlog age and throughput, manual prompts required (target zero for recoverable cases), stale-evidence rejection and recovery from crash.

## Evidence sources to verify freshly
- https://majordomus.dev/docs/integration/
- https://majordomus.dev/docs/mesh/
- https://majordomus.dev/docs/surfaces/
- https://majordomus.dev/docs/ci/
- https://majordomus.dev/docs/how-it-works/
- https://github.com/korczis/prismatic-majordomus
