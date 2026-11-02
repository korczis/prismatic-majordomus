# PROMPT 06 — Efficient deterministic derivation and projection

Goal: shrink expensive derivation while removing duplicated outputs and preserving correctness. Do not implement a general Merkle engine unless measured need justifies it.

1. Profile existing `scripts/derive` or actual canonical derive command and enumerate typed source inputs, generated targets, existing caches and dependencies. Identify duplicated parsing, excessive scans, full-tree invalidation, unstable sort and write-on-no-change.
2. Build a real dependency map per derived output. Where appropriate add normalized content fingerprints and correct cache invalidation with schema version, toolchain and generator code identity. Use atomic temp publication and rollback on failures.
3. Define total deterministic ordering for exposed discovered collections from canonical metadata with stable ID tie-breakers. A new rule/capability/service should not require manual registration in CLI/API/MCP/docs/UI.
4. Demonstrate cold vs warm, irrelevant input mutation vs dependent mutation, concurrent invocation, malformed input, schema bump and output identity. Compare committed generated artifacts before/after no-op run; no unrelated noise.
5. Refactor duplicates carefully, not in one gigantic rewrite. Leave existing protocol compatibility when consumers depend on stable ordering/IDs.
6. Update governance rules only when backed by executable validator; integrate existing `doctor/watch` categories and test cases.

## Acceptance
No stale output on relevant changes; deterministic stable no-op; measured improvement or remove optimization if neutral/regressive; zero new consumer inventories and clean generated drift check.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
