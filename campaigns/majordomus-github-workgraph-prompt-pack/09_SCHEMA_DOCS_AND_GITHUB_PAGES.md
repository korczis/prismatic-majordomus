---
id: github-workgraph-phase-09
phase: 9
depends_on: [github-workgraph-phase-08]
goal: schemas, documentation, examples, GitHub Pages
---

# Phase 09 — Schema, Documentation, Generated References, and GitHub Pages

Read shared contract and prior handovers.

## Objective

Make the architecture understandable and self-maintaining without creating a new forest of manually synchronized docs.

## Schemas

Ensure canonical typed schemas cover:
- work IDs;
- outcome/milestone;
- execution contract;
- dependency edges;
- acceptance criteria;
- evidence;
- external GitHub refs;
- authority/provenance;
- reconciliation status/operations;
- diagnostics.

Generate JSON/OpenAPI schemas through existing mechanisms.

Schema versioning and compatibility must follow repository practice.

## Architecture docs

Document:
1. why GitHub is projection/observation, not second truth;
2. Work Graph boundaries;
3. field-level authority;
4. identity mapping;
5. state/readiness/completion derivation;
6. reconciliation plan/apply lifecycle;
7. Git/worktree/commit/PR traceability;
8. evidence;
9. security/permissions;
10. offline behavior;
11. failure/recovery.

Use diagrams where current docs conventions support Mermaid or similar.

## Operator docs

Include real commands discovered from the implementation for:
- inspect work;
- next ready;
- explain blockers;
- GitHub status;
- diff/plan;
- reconcile;
- troubleshoot mapping/permissions;
- validate;
- migrate/adopt legacy.

Do not document conceptual command names that were never implemented.

## Extension guide

Explain how to add:
- a new work metadata field;
- a new evidence source;
- a new GitHub projected field;
- a new capability;
- potentially a future provider.

Emphasize zero consumer registration.

## GitHub Pages/site

Integrate generated/reference content into the existing site:
- architecture;
- capability reference;
- Work Graph concepts;
- GitHub setup;
- troubleshooting;
- use-case walkthrough.

Do not hand-copy capability lists. Generate them from canonical metadata.

## Examples

Add at least one end-to-end example fixture showing:

`Outcome M → Issues A/B/C → dependency → worktree → commits → PR → checks → acceptance`

and demonstrate reverse traversal.

Examples must be executable/test-backed if project conventions support executable use cases.

## README/landing page

Update high-level references only as warranted. Keep landing concise; link to generated detail.

If repository policy automatically derives landing improvements from capabilities/use cases, use that mechanism instead of manual feature marketing.

## Gate

Docs and site builds must pass drift/link/schema checks. Every command/API example shown must correspond to real generated capability semantics.
