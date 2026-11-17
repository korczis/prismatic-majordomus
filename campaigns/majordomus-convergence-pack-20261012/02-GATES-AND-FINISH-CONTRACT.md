# PROMPT 02 — Make finish mean finished

Goal: eliminate false-green finish outcomes across Rust, docs, UI, derived data and release. Reuse existing quality machinery; root-cause historical missed findings.

1. Map gates and dispatch: cargo fmt/clippy/test/doc examples/quality, shell or Rust cases, scripts/site-build/site-check, derivation drift, schema, API/MCP contract, UI snapshot and branch protection checks. Distinguish local, PR, merge, release and deploy gates.
2. Identify test selection from change impact. Build conservative dependency-aware route from changed files to required checks, with cross-cutting global gates always enforced. Unknown impact should select a safe superset or report UNKNOWN, never zero. Avoid large novel DSL.
3. Specifically simulate a change like PR #1041 that left 17 quality findings; prove the corrected finish contract catches it without actually breaking master. Add a regression fixture/unit/integration test.
4. Detect stale PASS: bind checks to git head SHA, toolchain identity, relevant inputs, schema/cache version and result timestamp. A successful check from a previous PR head is not current.
5. Produce machine-readable structured verdict PASS/FAIL/PENDING/STALE/UNKNOWN/BLOCKED with per-gate provenance, explanation and recommended fix. Extend canonical types if available rather than inventing UI-specific models.
6. Wire into existing CLI / CI / MCP / UI through the canonical projection. Add cross-surface tests only where those consumers currently exist.
7. Make focused checks fast via change selection but ensure required integration gates remain non-skippable. Never silence rules or change expected results to greenwash.

## Acceptance
At least 1 regression test reproduces the escaped-gate class; relevant canonical checks pass on HEAD; PR gate list and local finish verdict agree; no new manual duplicated check arrays. Docs and `.ai` enforcement updated only through existing governing source.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
