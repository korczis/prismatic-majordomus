---
analysis:
  id: cache-misses
  pass: lens
  lens: cache
---

# Lens Q1: cache

## Summary

The cache is process memory and every deploy starts new processes.

## Findings

| ID | Claim | Tag | Evidence |
|---|---|---|---|
| cache-F1 | The cache lives in process memory and is not persisted | OBSERVED | `src/cache.rs:12` |
| cache-F2 | A deploy restarts every process within two minutes | OBSERVED | `deploy.log` lines "restart" at 10:02-10:04 |
| cache-F3 | The eviction policy also drops hot keys | INFERRED | a run with eviction disabled would show fewer misses |

## Gaps ledger

- The production trace was not exported, so per-key hit rates were not read.
