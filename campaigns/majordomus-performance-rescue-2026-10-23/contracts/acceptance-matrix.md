# Acceptance Matrix: no pass without attached evidence

Mark each row **PASS / FAIL / BLOCKED / NOT_RUN** with SHA, command, sample N, machine/profile, evidence URI/path and timestamp. PASS requires an executed test, not a prose conclusion.

| Test | Scenario | Required proof | Status |
|---|---|---|---|
| CLI entry | `bin/majordomus` vs direct Rust | cold/warm process wall-clock & exit contract | NOT_RUN |
| direnv | repeated `cd` / first entry / stale binary | per-phase timing, zero build/network | NOT_RUN |
| runtime | absent/current/outdated server | lease identity, time-to-usable route | NOT_RUN |
| `/ui` cold | empty readings and slow first source | usable truthful HTML without 60s wait | NOT_RUN |
| `/ui` warm | all discovered screen routes | existing p95 budget and no reindex | NOT_RUN |
| `/cockpit` cold/warm | overview, catalog, graphs, details | SSR + browser timing + source provenance | NOT_RUN |
| UI panel | delayed `integration.queue` | page paints independently, late result updates | NOT_RUN |
| `plan.flight` | warm then Git change and mesh claim | no recurring Git-history scan; correct updates | NOT_RUN |
| tracing | overlapping HTTP requests | request-local Server-Timing / trace attribution | NOT_RUN |
| concurrency | slow handler + assets + websocket | bounded resources, fast UI/probe or timely 503 | NOT_RUN |
| realtime | event -> screen + reconnect | measurable DOM latency & honest stale states | NOT_RUN |
| browser | desktop/mobile/no-JS | FCP/LCP/interaction, console, a11y | NOT_RUN |
| cache | worktree switch, source mutation | provenance and equivalence; no cross-repo leak | NOT_RUN |
| scale | fixtures 1x/10x/100x | measured slope and memory bounds | NOT_RUN |
| chaos | dead peer/forge, stalled source | bounded response and truthful degraded state | NOT_RUN |
| enforcement | inject a known regression | canonical gate fails, precise diagnostic | NOT_RUN |
| CI | canonical tests and generated checks | links to passing required jobs | NOT_RUN |
| hosted live | authoritative hosting | deployed revision + browser/live socket | NOT_RUN |
| GH Pages | static published snapshot | revision+staleness+links+assets verified | NOT_RUN |
| release | merge train | exact integrated SHA, no skipped safety gate | NOT_RUN |
