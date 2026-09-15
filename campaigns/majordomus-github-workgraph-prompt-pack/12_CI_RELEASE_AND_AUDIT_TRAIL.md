---
id: github-workgraph-phase-12
phase: 12
depends_on: [github-workgraph-phase-11]
goal: CI, release evidence and audit trail
---

# Phase 12 — CI, Release Traceability, and Audit Trail

Read shared contract and prior handovers.

## Objective

Make Work Graph integrity part of normal repository operation and connect accepted work to release evidence where appropriate.

## CI integration

Use existing workflows/gates. Add the smallest canonical integration that ensures:
- schema validity;
- Work Graph validity;
- generated projection drift;
- capability/use-case coverage;
- unit/integration/E2E tests;
- frontend/Cockpit tests where existing CI supports them;
- documentation/site checks.

Do not invent a parallel mega-workflow if `just check`/canonical gate already feeds CI.

## Remote-dependent validation

Split:
- deterministic offline required checks;
- optional/live GitHub checks;
- credentialed reconciliation status checks if policy demands them.

CI should fail clearly on permission/configuration mistakes without dumping secrets.

## Pull request checks

If appropriate, expose Majordomus-specific check summaries:
- traceability valid;
- work item mapping;
- acceptance/evidence coverage;
- generated artifacts synced;
- dependency state.

Do not mark an issue complete merely because the PR check is green.

## Audit trail

Use existing session/handover/audit infrastructure if present.

Record structured events for meaningful actions:
- external observation refreshed;
- reconciliation planned;
- reconciliation applied;
- remote mutation result;
- mapping adopted/changed;
- work accepted;
- release linked.

Each event should include safe identifiers, actor/provider if known, timestamp, before/after hashes or semantic diff where appropriate, but no secrets/raw unnecessary personal data.

Audit events must be append-oriented/history-preserving if current architecture supports it.

## Release traceability

Inspect current semantic versioning/changelog/release architecture.

Where it fits, derive:
- accepted work items included in a release;
- outcomes/milestones advanced/completed by release;
- PRs/commits included;
- verification evidence.

Do not duplicate the changelog source. Integrate with canonical release/changelog pipeline.

A release should be able to answer:
`what accepted work and evidence produced this artifact?`

And a work item:
`in which release did this land?`

Only implement relations that can be derived reliably from git/release data.

## GitHub release projection

If GitHub Releases are in scope and existing release tooling supports them, project generated release notes/references from canonical release/changelog data, not a separate manual document.

## Gate

CI must enforce the local deterministic invariants. Release traceability must be derived from canonical/git evidence and remain testable.
