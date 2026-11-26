---
analysis:
  id: cache-misses
  pass: synthesis
---

# Synthesis: why the read cache misses after every deploy

Built from CONFIRMED findings only.

## Root causes

| RC | Cause | Findings |
|---|---|---|
| RC-a | A deploy restarts every process, and the cache dies with its process | cache-F1, cache-F2 |
| RC-b | Every miss in the cold window costs a full read | load-F2 |

## Remedies

| ID | Remedy | Addresses | Owner | Verified by |
|---|---|---|---|---|
| R1 | Warm the cache from a snapshot at start | RC-a | the cache maintainers | the miss rate in the ten minutes after a deploy |
| R2 | Roll processes one at a time | RC-a, RC-b | unowned | the latency after a deploy |

## Corrected claims

| ID | Verdict | Correction |
|---|---|---|
| load-F1 | REFUTED | 20% of reads miss, not 50%: retries were counted twice |

## Gaps ledger

- cache-F3: whether eviction drops hot keys needs a run with eviction disabled.
- The production trace was not exported.
