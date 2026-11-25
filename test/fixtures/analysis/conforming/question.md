---
analysis:
  id: cache-misses
  pass: question
---

# Question: why does the read cache miss after every deploy?

Written before any lens reads anything.

## Questions

- Q1 Cache: what empties the cache, and when?
- Q2 Load: how much of the read latency do the misses explain?

## Rules

- Read-only: no deploy, no restart, nothing written outside this bundle.
- Every claim is a row of a findings table, tagged OBSERVED with a locator, DERIVED with a
  formula or INFERRED with what would confirm it. An untagged claim is invalid.
- An unrun check is reported as not run. A missing source goes into the gaps ledger.

## Passes

1. One lens per question, independently.
2. Each lens cross-checks the other's findings: CONFIRMED, REFUTED or UNVERIFIABLE.
3. A synthesis from CONFIRMED findings only, remedies mapped to owners.
