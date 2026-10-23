# Existing Issues and Single Release Train

These are **current as of the GitHub review 2026-10-23**, recheck status/ownership before acting:

| Canonical | GitHub | Status at review | Integration |
|---|---|---|---|
| I3226 | https://github.com/korczis/prismatic-majordomus/issues/1564 | open READY | request-scoped Server-Timing, extend existing perf instrumentation |
| I3442 | https://github.com/korczis/prismatic-majordomus/issues/1805 | open READY | dependency-keyed plan.flight cached inference; avoid duplicate work in an active claim |
| I3299 | https://github.com/korczis/prismatic-majordomus/issues/1630 | open READY | adopting-repo overhead and budgets |
| I3227 | https://github.com/korczis/prismatic-majordomus/issues/1565 | open BLOCKED | prefetch only if I3226 proves navigation latency |
| I3140 | https://github.com/korczis/prismatic-majordomus/issues/1486 | open BLOCKED | browser visual latency and reconnect baseline, dependencies must be respected |
| I2486 | https://github.com/korczis/prismatic-majordomus/issues/1073 | closed DONE | preserve latest-readings / pending / async update contract |
| I2489 | https://github.com/korczis/prismatic-majordomus/issues/1076 | closed DONE | preserve warm UI p95 100ms gate and ensure it actually runs |

This user complaint is P0 even if existing backlog records are lower priority. Respect project governance to reprioritize, link and merge into one train; no false reassignment without authorization.

## Integration order

1. Reproduce and trace with current issue map.
2. Repair cold UX/readings/starvation and prove correct async update.
3. Repair CLI/direnv critical path.
4. Repair repeated `plan.flight`/other measured heavy derivations, preserve fresh overlays.
5. Repair HTTP/WS contention if evidence supports.
6. Browser + benchmark end-to-end gates.
7. Docs/derived surfaces, CI, deploy, independent verification, train merge.

Every issue closed must have a link to acceptance evidence and code + test. No splitting a release train into unmergeable independent feature PRs. Use the repository's own `majordomus plan`, `majordomus prs`, task lifecycle and merge permissions as actually supported on HEAD. A history of source already merged is not new progress.
