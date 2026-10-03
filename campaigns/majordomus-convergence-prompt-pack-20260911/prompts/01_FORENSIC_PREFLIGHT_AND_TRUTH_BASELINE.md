# Stage 01 — Forensic Preflight and Truth Baseline

Do not implement features yet. Establish a trustworthy current state and machine-readable convergence ledger.

## Tasks

1. Read repository instructions/governance and current session/handover context.
2. Record branch, HEAD, remotes, status and all worktrees.
3. Inspect local branches/unfinished work and remote state if available.
4. Discover the repository's canonical quality/check commands from `just`, scripts and CI. Do not invent a parallel command set.
5. Run the fastest structural gates first; then progressively broader tests that the environment supports.
6. Inventory all public commands and classify them by effect (`query`, `mutation`, execution/control, etc.) from canonical metadata or behavior.
7. Inventory canonical capabilities and compare public mutating operations with capability backing.
8. Parse all project/vendored rules/doctrines/policies through the project's own parser/schema. Record enforcement metadata, validators, tests, claims and gate reachability.
9. Inventory every debt/baseline/allowlist file related to canonical runtime, ordering, hardcoding, coverage, generated data, lifecycle or claim drift.
10. Inventory provider definitions and actual capture/session lifecycle adapters.
11. Inventory Cockpit area/nav/routes and determine their canonical source(s).
12. Inventory guaranteed claims and what counts as current evidence.
13. Inventory generated artifacts and drift checks.
14. Produce a dependency DAG for stages 02–15 based on current facts.

## Required outputs

Create/update an evidence report and debt ledger. Every entry must include:

```text
ID
invariant
canonical source
observed violating artifact
reproduction command
current gate coverage
target exit test
stage owner
```

## Acceptance

- No major audit conclusion is based only on prose.
- All baseline numbers in later stages come from the current checkout.
- You can explain which source is canonical for commands, capabilities, rules, provider metadata, Cockpit navigation, claims and generated site data.
- You know exactly which structural gates are currently red before any migration begins.

Commit only if the repository convention expects audit/evidence artifacts to be tracked; otherwise keep evidence in the existing session/handover mechanism.
