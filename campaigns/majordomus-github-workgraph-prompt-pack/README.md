# Majordomus GitHub / Work Graph Prompt Pack

This pack is designed for a large-context coding agent (Claude Code Fable/Opus class, ~1M context) working directly in `~/dev/prismatic-majordomus`.

It is deliberately **multi-phase**. A single megprompt would encourage broad edits, forgotten invariants and a glorious new sedimentary layer. Run the prompts in order.

## Recommended execution

1. Place/extract this directory somewhere outside the implementation worktree, or under a temporary ignored path.
2. Start Claude Code at the canonical repository checkout.
3. Give it `00_SHARED_CONTRACT.md` first and tell it that the file is normative for the campaign.
4. Run `01` through `13` in order.
5. Do not skip a phase merely because an adjacent feature "looks implemented". Each phase must inspect the current repository and prove the invariant with code/tests.
6. Let the repository's own worktree/task/session rules decide exact branch naming, commits, generation and handovers.
7. If a later phase discovers a defect in an earlier invariant, fix it rather than papering over it.

## Files

- `00_SHARED_CONTRACT.md` — campaign-wide architecture and discipline.
- `01_DISCOVERY_AND_GAP_AUDIT.md` — evidence-based audit and executable target plan.
- `02_CANONICAL_WORK_GRAPH.md` — typed milestones/issues/dependencies/evidence graph.
- `03_GITHUB_PROVIDER_AND_PROJECTION.md` — GitHub boundary, identity mapping and provider model.
- `04_RECONCILIATION_ENGINE.md` — bidirectional synchronization, drift/conflicts and idempotence.
- `05_GIT_WORKTREE_COMMIT_PR_TRACEABILITY.md` — tri-directional traceability through Git/worktrees/PRs.
- `06_GOVERNANCE_AND_ENFORCEMENT.md` — rules, doctrines, policies, doctor/check/finish gates.
- `07_CAPABILITIES_CLI_API_OPENAPI_MCP.md` — one capability model projected to machine/user surfaces.
- `08_COCKPIT_CONTROL_PLANE.md` — graph/status/drift/evidence UI without frontend-domain duplication.
- `09_SCHEMA_DOCS_AND_GITHUB_PAGES.md` — schemas, docs, examples and generated site.
- `10_LEGACY_MIGRATION_AND_BACKFILL.md` — adopt existing issues/milestones/branches/PRs safely.
- `11_E2E_CONTRACT_AND_CHAOS_TESTS.md` — real lifecycle, property/contract/negative/idempotence tests.
- `12_CI_RELEASE_AND_AUDIT_TRAIL.md` — CI enforcement, release evidence and auditability.
- `13_ADVERSARIAL_FINAL_REVIEW.md` — attack the implementation, remove duplication, prove completion.
- `RUN_ALL.md` — orchestration prompt for an agent that can execute the whole campaign continuously.

## Core end-state

The system should converge on a single typed relationship graph:

```text
Outcome/Milestone
    │
    ├── contains ──> Issue / Execution Contract
    │                   │
    │                   ├── depends_on ──> Issue
    │                   ├── owns ──> Branch
    │                   ├── executes_in ──> Worktree
    │                   ├── realized_by ──> Commits
    │                   ├── proposed_by ──> Pull Request
    │                   └── satisfied_by ──> Evidence / Checks
    │
    └── accepted only when derived completion invariants hold
```

GitHub is a projection/collaboration plane, not a second hand-maintained database of Majordomus semantics.

## Hard rule

If the implementation requires someone to add the same command, issue category, mapping, route, UI list item, docs row or MCP operation in multiple places, stop and repair the canonical source/projection architecture.
