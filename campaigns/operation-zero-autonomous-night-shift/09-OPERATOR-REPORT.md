# 09 — Autonomous status and final report

No routine questions. Do not ask operator to approve trivial filenames, naming, branch rebases or ordinary implementation decisions. Continue while runtime is available; preserve state if interrupted.

Update durable operational snapshot at every transition. Human-readable periodically, minimal noise:

```
OPERATION ZERO | UTC timestamp
PRs: initial N | open K | merged M | reconciled C | externally blocked B
Queue: ready R | in repair W | in integration 0/1
Current: #number / phase / SHA
Versions: reserved X | released Y | deployed/verified Z | outstanding Q
Throughput: verified merges/hour, p50/p95 merge-to-deploy
Next: #number | blocker if any
```

Final report must distinguish VERIFIED, PARTIAL and BLOCKED. Include exact authoritative counts, URLs, checks, release/tag and deployed commit proof, recoverable journal location and replay action, worktree cleanup result, and remaining obstacles. Never imply continuing execution after the runtime has actually terminated. The operator should be able to wake up and see what happened without reading 5000 lines of logs.
