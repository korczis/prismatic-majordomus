# Phase 19: Acceptance Criteria, Cross-Surface Traceability and Final Proof

## Core acceptance records
AC-001 intent statement and explicit measurable criteria persist with provenance.
AC-002 planner finds genuine gaps and does not silently invent acceptance criteria as user-approved.
AC-003 critique can reject unsafe/inadequate plan and records reason.
AC-004 approved plan revision creates typed DAG without cycles or orphan criteria.
AC-005 scheduler respects dependencies and bounded WIP.
AC-006 mesh worker selected via actual capabilities/health/claim semantics.
AC-007 conflicting scopes not simultaneously owned as valid exclusive claims.
AC-008 execution event replay matches persisted state after restart.
AC-009 stale workers fenced out of effects.
AC-010 duplicate request/webhook produces no duplicate external side effect.
AC-011 pause/resume/cancel controls are authorized and reversible where feasible.
AC-012 UI status/event stream recovers after disconnect without lying.
AC-013 worktree changes preserve other people's unmerged work.
AC-014 PR ready verdict includes mandatory checks, protected branch and actual review.
AC-015 merge state observed from GitHub, not inferred from request receipt.
AC-016 release version derived from actual versioning policy and public API diff.
AC-017 release uses unique version/source SHA; retry reconciles partial side effects.
AC-018 deployment verifies expected artifact SHA and observed runtime/Pages target.
AC-019 failed health smoke blocks satisfaction and appropriate promotion.
AC-020 criterion verdict uses trusted, fresh, subject-matched evidence.
AC-021 revoked/stale evidence correctly changes derived satisfaction.
AC-022 unauthorized effect denied across all transports.
AC-023 `.ai/local` never published in public docs/API samples.
AC-024 all public operations derived from one canonical capability source.
AC-025 CLI/API/OpenAPI/MCP/UI agree on ID, schema, error and permission.
AC-026 generated docs/navigation have zero per-consumer registration.
AC-027 live UI works at mobile viewports, keyboard and reconnect.
AC-028 benchmark results do not regress established hard gates.
AC-029 dogfood on disposable repo proves end-to-end completion.
AC-030 guarded real-repo scenario demonstrates authorization and safe recovery.
AC-031 each milestone/issue maps to originating intent criterion or documented justified infrastructure need.
AC-032 explain graph provides intent -> plan -> issue -> attempt -> commit -> PR -> release -> deploy -> evidence.
AC-033 claims of production-ready autonomy link to exact trace and observed deployment proof.
AC-034 failed service or missing external permissions are transparent blocked states, not false success.
AC-035 no duplicated independent scheduler, catalog, policy engine, event log or UI status model added.

## Reviewer protocol
For each AC create or reuse an executable use case and link:
- canonical implementation path/owner and capability identifier
- source assertion/test fixture
- use-case scenario ID
- CLI/API/MCP/UI projection where applicable
- CI run and source SHA
- evidence artifact with hash
- observed result and unknowns
Machine-check these links where repository already supports coverage.
Do not add a giant static manual duplicate spreadsheet; this file is a mandate checklist. Materialize traceability through canonical use-case/intent/evidence records.

## Final decision
PASS requires AC-001..035 relevant to declared delivery scope with actual evidence. If deployment target/permissions unavailable, status is PARTIAL: sandbox E2E verified, external deployment pending authorization. Never substitute mock evidence for live production proof.
