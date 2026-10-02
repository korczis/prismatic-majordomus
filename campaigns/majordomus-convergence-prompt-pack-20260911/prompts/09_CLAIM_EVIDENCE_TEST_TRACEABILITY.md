# Stage 09 — Claim → Rule → Test → Evidence Traceability

Make Majordomus prove what it says, at the landed commit.

## Mission

Upgrade claims from “linked documentation and test paths” to current executable evidence.

## Canonical graph

Implement/reuse a graph such as:

```text
claim
 ├─ status: guaranteed/advisory/planned/...
 ├─ obligations
 │   ├─ rule IDs
 │   ├─ capability/implementation IDs
 │   └─ surface obligations
 ├─ test IDs / gates
 └─ evidence
     ├─ commit SHA
     ├─ run/build ID
     ├─ timestamp
     ├─ result
     └─ artifact/log reference
```

Do not duplicate CI vendor data inside the repo if an immutable reference is sufficient. The model should support local evidence and CI evidence.

## Tasks

1. Inventory guaranteed claims and identify those whose “evidence” is only a file path or stale static assertion.
2. Define executable obligation IDs and connect tests/gates to them.
3. Emit machine-readable test/gate results keyed by obligation when canonical checks run.
4. Aggregate claim status from the current evidence of all required obligations.
5. Mark evidence stale when it does not correspond to the relevant current code/data revision according to repository policy.
6. Expose claim proof via CLI/API/MCP/Cockpit/site.
7. Ensure site/landing page cannot render a guaranteed/current feature when obligations are missing/red/stale.
8. Add mutation tests for a broken implementation, missing test, stale evidence and planned feature accidentally promoted to guaranteed.

## Acceptance

- Every guaranteed claim has executable obligations.
- Current status is derivable, not manually typed into multiple docs.
- Breaking a required test makes the associated guaranteed claim visibly unverified/red.
- GitHub Pages/Cockpit show truth without maintaining duplicate claim state.
- Evidence links to the exact landed revision/run where feasible.
