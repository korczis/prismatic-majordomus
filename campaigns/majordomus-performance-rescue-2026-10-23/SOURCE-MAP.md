# Source map: leads, not live measurements

Verified against the GitHub-connected default branch on 2026-10-23. Everything must be rechecked against local HEAD. Source comments and existing issue bodies may describe historical performance; do not treat them as current benchmarks.

| Area | Path or link | Observed code / existing issue |
|---|---|---|
| repo | [https://github.com/korczis/prismatic-majordomus](https://github.com/korczis/prismatic-majordomus) | repo master; release merge 575d39f8d24f21ed4c5615c9f39d3b0ff624323e at review time |
| reading | [apps/majordomus-cli/src/capability/readings.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/capability/readings.rs) | PAGE cold 60s, moved 50ms; 4 source workers; 1s REVALIDATE; 512 sources/300s remember; priority queue |
| UI | [apps/majordomus-cli/src/ui/mod.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/ui/mod.rs) | Ui::warm_once warms all screens/panels; handle uses Live::view_without_waiting |
| UI sources | [apps/majordomus-cli/src/ui/source.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/ui/source.rs) | ask_all spawns per-source and joins; uses read_shared and capability describe |
| dashboard | [apps/majordomus-cli/src/ui/dashboard.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/ui/dashboard.rs) | 7 first paint sources; integration.queue deferred; 2026-10-11 historical 50s queue comment |
| env | [apps/majordomus-cli/src/commands/env.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/commands/env.rs) | fast environment + policy, runtime ensure and refresh_bridge; some commands full App::load |
| shell | [bin/majordomus](https://github.com/korczis/prismatic-majordomus/blob/master/bin/majordomus) | Bash lifecycle CLI coexists with Rust executable |
| env bridge | [bin/majordomus-env](https://github.com/korczis/prismatic-majordomus/blob/master/bin/majordomus-env) | checks binary freshness; no build on cd |
| entry | [.envrc](https://github.com/korczis/prismatic-majordomus/blob/master/.envrc) | calls bin/majordomus-env enter; watches runtime lease |
| server | [apps/majordomus-cli/src/http/server.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/http/server.rs) | 4 accepts, max64 handler threads; overloaded requests 503 except GET / |
| cockpit | [apps/majordomus-cli/src/cockpit/mod.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/cockpit/mod.rs) | Cockpit SSR and navigation from Live::current |
| perf | [apps/majordomus-cli/src/perf.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/perf.rs) | global atomic counters; lacks request-local stage attribution in reviewed issue |
| policy | [.ai/repo/benchmarks/rust/policy.yaml](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/benchmarks/rust/policy.yaml) | warm /ui budget p95 100ms, min20 samples; platform-specific regression policy |
| hot path | [apps/majordomus-cli/tests/hot_path.rs](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/tests/hot_path.rs) | 200 HTTP/stdio calls do not rebuild canonical state |
| issue I3226 | [https://github.com/korczis/prismatic-majordomus/issues/1564](https://github.com/korczis/prismatic-majordomus/issues/1564) | Open; per-request Server-Timing requested |
| issue I3442 | [https://github.com/korczis/prismatic-majordomus/issues/1805](https://github.com/korczis/prismatic-majordomus/issues/1805) | Open; plan.flight repeated full Git history inference |
| issue I3299 | [https://github.com/korczis/prismatic-majordomus/issues/1630](https://github.com/korczis/prismatic-majordomus/issues/1630) | Open; adoption overhead benchmark |
| issue I3140 | [https://github.com/korczis/prismatic-majordomus/issues/1486](https://github.com/korczis/prismatic-majordomus/issues/1486) | Open; browser paint/reconnect baseline |
| issue I3227 | [https://github.com/korczis/prismatic-majordomus/issues/1565](https://github.com/korczis/prismatic-majordomus/issues/1565) | Open; prefetch only after measured value |
| issue I2486 | [https://github.com/korczis/prismatic-majordomus/issues/1073](https://github.com/korczis/prismatic-majordomus/issues/1073) | Closed; historic 2.2-2.5s /ui, 75.7s panel, latest-reading rework |
| issue I2489 | [https://github.com/korczis/prismatic-majordomus/issues/1076](https://github.com/korczis/prismatic-majordomus/issues/1076) | Closed; warm UI p95 budget |
| browser | [scripts/lib/browser.mjs](https://github.com/korczis/prismatic-majordomus/blob/master/scripts/lib/browser.mjs) | canonical Playwright launch used by scripts/ui |
| UI automation | [scripts/ui](https://github.com/korczis/prismatic-majordomus/blob/master/scripts/ui) | pages, viewports, check, audit, navbar, test |
| docs | [docs/PERFORMANCE.md](https://github.com/korczis/prismatic-majordomus/blob/master/docs/PERFORMANCE.md) | MJ_TIMING and benchmark workflow; numeric claims need runs |
