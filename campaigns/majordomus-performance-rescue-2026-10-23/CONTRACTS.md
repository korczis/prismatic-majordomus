# P0 Performance Contracts

## What counts as responsiveness

1. **CLI:** time the executable entry, not merely the handler; shell and Rust must be measured separately. A command that explicitly asks for an exact full build/recompute is separately classified and must not be silently turned into stale status.
2. **Cold UI:** fresh process, fresh readings queue, no capability cached; useful SSR and honest pending/stale states must appear promptly even with indefinitely slow remote sources. Wall-time includes accept/queue, render, response, CSS/JS and browser paint.
3. **Warm UI:** retain the existing `<100 ms p95` warm SSR budget from tracked benchmark policy, with required sample count. A synthetic benchmark in a pinned context does not establish true end-to-end UX.
4. **Cockpit:** full progressive SSR content, derived navigation and provenance, no external app or new catalog. Tests cover heavy views and details.
5. **Live:** data freshness and source identity carry through change/reconnect; stale UI does not become falsely green. Measure actual event-to-DOM, not server send time.
6. **Authority:** stale snapshots may satisfy read-only display; writes and safety-critical decisions demand authoritative verification. Cross-worktree/source identity in cache keys.
7. **Security:** no changed trust boundary to win latency, no reduced origin/CSP/signed mesh checks, no secret-bearing telemetry.
8. **No duplication:** extend `perf`, `bench`, `Live`, `readings`, canonical capability registries, site and documentation generation. No second stores, schedulers, event buses, browser inventories.

## Provisional user-facing targets, not claimed observations

| Path | Target | Interpretation |
|---|---|---|
| `--help` process cold | p95 <= 50ms | compiled executable / shell adapter measured separately |
| cheap local CLI read | p95 <= 100ms | only a contractually cheap command; full exact scan excluded |
| warm `/ui` server SSR | p95 <= 100ms | already committed benchmark policy |
| cold `/ui` useful SSR | p95 <= 1000ms | fresh process/readings, honest pending state |
| warm user interaction | p95 <= 150ms | actual browser response |
| local event to visible DOM | p95 <= 250ms | on controlled environment |

Never encode a new target as hard gate without a measured baseline, project policy review and enough samples. If owner target cannot be reached without lying or breaking correctness, mark FAIL/UNMET with evidence.

## Anti-goals

No framework migration for fashion; no React/Phoenix/Tokio replacement purely for speed claims; no service worker/cache that hides stale data; no GitHub API call on direnv; no unbounded `spawn`; no hardcoded source status in UI; no silent timeouts turned into successful zeros; no prefetching every expensive panel; no force push; no disabling quality gates; no manual baseline edits; no fake CI/browser/deploy evidence.
