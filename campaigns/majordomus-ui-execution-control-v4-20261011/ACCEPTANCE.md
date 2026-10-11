# End-to-end acceptance matrix

| Journey | Proof required |
|---|---|
| Start a real issue in UI | readiness from canonical issue, authority verified, durable execution ID, linked parent milestone/intent |
| Start a real task | task is claimed once, policy/evidence recorded, worker shown by authoritative source |
| Start a milestone | eligible issues launched per DAG, dependent issues wait, scope revisions tracked, group run inspectable |
| Start an intent | intent criteria and milestones visible, verified satisfaction only after evidence, no false complete |
| Live progress | real events, order/cursor/replay/reconnect, log links, timestamps and provenance |
| Browser closed mid-run | work continues if durable policy authorizes; browser reload restores real state |
| Server and worker crash | recovery or explicit blocker with evidence; no duplicated mutation |
| Pause/resume/cancel/retry | only when implementation supports it, idempotent and authorized, exact state reflected |
| Deep link | copy `/cockpit/...` URL, reload, back, keyboard and mobile navigation |
| Cross-surface parity | same execution ID, status and denial in CLI/MCP/API/UI |
| Security | unauthorized launch/subscription denied; CSRF/origin/credentials handled; secrets absent from payloads |
| No mock data | production has no statically embedded PRs, peers, progress, metrics or synthetic events |
| Gates | regression fails actual required CI job; cannot bypass through alternate UI endpoints |
| Docs and public site | generated docs and live UI features linked to real code/evidence, version/freshness matched |
| Release | merged SHA, version analysis, signed/provenance verified artifacts, deployed URL and external smoke result |

## Metrics

Measure time to initial truthful SSR and to first live update; websocket reconnect recovery; max event lag; event loss after restart; readiness/launch latency; browser CPU/memory on large graph; task end-to-end wall clock. Record distributions rather than invented numeric SLO claims until a baseline exists. Specify budgets in canonical performance policy with test evidence.

## Manual walkthrough

Open UI, select real intent, inspect criteria, preview run, launch with approved authority, watch milestones/issues/tasks start, pause browser, reconnect, inspect true logs, trigger a safe failure, inspect blocker, recover and reach evidence-backed complete. Reproduce from CLI and MCP, then verify cross-surface IDs. Repeat in independent consumer repo.
