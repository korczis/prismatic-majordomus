---
analysis:
  id: cache-misses
  pass: cross-check
  lens: cache
---

# Cross-check by the cache lens: load findings

| ID | Verdict | Tag | Evidence |
|---|---|---|---|
| load-F1 | REFUTED | DERIVED | 2,000 misses / 10,000 reads: `metrics.csv` counts a retried read twice |
| load-F2 | CONFIRMED | DERIVED | recomputed: 52 ms - 12 ms = 40 ms |

## Gaps ledger

- none
