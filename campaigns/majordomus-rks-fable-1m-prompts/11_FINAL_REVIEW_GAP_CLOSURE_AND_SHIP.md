# Phase 11 — Final Cross-Cutting Review, Gap Closure, and Ship

Use `00_MASTER_CONTRACT.md` as the final acceptance contract.

This is not a new feature phase. Your task is to inspect the completed RKS implementation end-to-end, find every place where reality still violates the architectural contract, close the gaps, and leave the repository release-ready.

## Step 1: independently re-audit

Do not trust prior implementation summaries. Re-read the actual code, schemas, docs, tests, generated interfaces and repo rules.

Check every master invariant:

- one canonical definition
- deterministic-first
- evidence before derived claims
- no silent drift
- brownfield baseline
- existing docs reused
- typed core / Markdown projection
- worktree awareness
- versioned schemas/migrations
- secure ingestion
- explainability

Produce an internal gap list before editing.

## Step 2: interface parity review

Build a capability matrix from actual implementation:

```text
capability                CLI API OpenAPI MCP Cockpit Docs Tests
bootstrap
status
show/explain
search
graph
impact
gaps
coverage
stale
conflicts
reconcile
baseline
validate/check
```

Do not force every capability into every interface if nonsensical, but every omission must be deliberate and documented.

Fix accidental gaps.

The matrix itself should be generated from metadata if Majordomus already has a capability registry suitable for this. Avoid creating a manual forever-matrix.

## Step 3: drift/duplication hunt

Search repository-wide for:

- duplicated knowledge kind lists
- duplicated freshness/provenance names
- duplicated route/CLI descriptions
- hand-maintained Swagger examples
- hand-maintained Cockpit menu entries
- repeated schema IDs
- duplicated docs navigation
- copied generated Markdown

Consolidate into canonical metadata.

## Step 4: dogfood from clean state

From a clean/temporary worktree:

1. run normal project bootstrap/init as a user would,
2. run RKS bootstrap/status,
3. inspect generated state,
4. run knowledge check,
5. make a controlled source change,
6. run impact,
7. verify stale state,
8. reconcile,
9. verify green state,
10. exercise representative API/MCP/Cockpit paths.

Record or encode this as reproducible E2E automation where practical.

## Step 5: docs truthfulness review

Verify every documented command, route, screenshot/description, schema and behavior exists in the shipped implementation.

Delete aspirational documentation that is not implemented. Future roadmap belongs in roadmap sections, not current capability docs.

## Step 6: product/DX review

Test the perspective of a developer integrating Majordomus into an unfamiliar legacy repository.

The first-run path should be:

- minimal questions
- clear discovery summary
- no destructive rewrite
- useful baseline
- immediate `status`/`explain` value
- obvious next action

Reduce friction that is not buying correctness.

## Step 7: failure mode review

Force and verify graceful behavior for:

- no git repository
- unsupported language mix
- malformed external docs/frontmatter
- stale/unsupported schema version
- missing provider credentials
- network unavailable
- corrupt cache
- conflicting evidence
- detached HEAD
- multiple worktrees
- ignored/vendor directories
- very large graph request

Errors should be actionable, not panics.

## Step 8: run strongest checks

Run the strongest practical suite available in this repository:

- formatting
- lint
- unit/integration
- schema validation
- E2E
- docs build
- site build
- API/OpenAPI validation
- MCP tests
- Cockpit tests
- relevant benchmarks

Fix failures rather than waving at them.

## Step 9: final architectural note

Update the canonical ADR/knowledge docs with the final actual architecture after implementation. If previous ADR text describes abandoned intermediate designs, supersede/update it using the repo’s normal process.

## Step 10: ship readiness summary

Provide a final concise report containing:

### Implemented

What RKS actually does now.

### Canonical sources

Which types/registries drive which projections.

### Brownfield behavior

What happens on adoption and how baseline works.

### Security

What can/cannot leave the machine and under what policy.

### Tested

Exact categories and commands run.

### Remaining limitations

Only real limitations, clearly separated from shipped behavior.

### Release readiness

State whether the implementation is safe to merge/release based on observed checks. If something blocks release, fix it if feasible within the repo rather than merely listing it.

## Absolute final acceptance criterion

The implementation is acceptable only if this statement is true in practice:

> A repository that adopts Majordomus late can build a useful knowledge model from real evidence, preserve and reuse existing documentation, detect when knowledge drifts as code changes, explain why it believes a claim, enforce that new debt does not increase beyond a brownfield baseline, and expose the same consistent state to developers, agents, CI and Cockpit without parallel manually-maintained registries.
