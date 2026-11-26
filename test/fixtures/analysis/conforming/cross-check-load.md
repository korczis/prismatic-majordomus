---
analysis:
  id: cache-misses
  pass: cross-check
  lens: load
---

# Cross-check by the load lens: cache findings

| ID | Verdict | Tag | Evidence |
|---|---|---|---|
| cache-F1 | CONFIRMED | OBSERVED | re-read `src/cache.rs:12`: a map in the process, no store |
| cache-F2 | CONFIRMED | OBSERVED | `deploy.log`: 6 restarts between 10:02 and 10:04 |
| cache-F3 | UNVERIFIABLE | INFERRED | needs a run with eviction disabled, which the rules forbid here |

## Gaps ledger

- cache-F3 needs an experiment outside this analysis's read-only rule.
