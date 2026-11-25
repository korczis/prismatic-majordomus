---
analysis:
  id: cache-misses
  pass: lens
  lens: load
---

# Lens Q2: load

## Summary

The misses explain most of the latency spike after a deploy.

## Findings

| ID | Claim | Tag | Evidence |
|---|---|---|---|
| load-F1 | Half of the reads miss in the first ten minutes after a deploy | DERIVED | 5,000 misses / 10,000 reads from `metrics.csv` |
| load-F2 | A miss costs 40 ms more than a hit | DERIVED | 52 ms - 12 ms = 40 ms, medians of `latency.csv` |

## Gaps ledger

- none
