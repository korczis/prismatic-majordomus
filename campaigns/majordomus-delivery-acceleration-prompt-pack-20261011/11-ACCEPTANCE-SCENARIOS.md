# EXECUTABLE ACCEPTANCE SCENARIOS (SPECIFICATION)

These are behavior specs, not claims that current commands exist. Discover and map to actual repo tool/test harness and names; avoid constructing a second test runner.

| ID | Scenario | Required result |
|---|---|---|
| ACC-01 | Same unchanged repo derive twice | byte-identical artifacts; second run either skips or explains why not |
| ACC-02 | Change a source affecting one projection | only verified affected closure regenerated |
| ACC-03 | Change generator/schema | stale cache invalidated, no false reuse |
| ACC-04 | Kill derive halfway | no falsely committed complete output; restart safe |
| ACC-05 | Concurrent derive across worktrees | lock/fencing or deterministic isolation; no corruption |
| ACC-06 | Full versus incremental derive | identical final output on representative input classes |
| ACC-07 | Ready PR after dependency merge | eligibility based on valid git/evidence semantics; no wasted refresh |
| ACC-08 | Same tree with revert/nonmerge history change | unsafe equivalence rejected |
| ACC-09 | Parallel PR drain workers | exactly-once merge effects / no double release |
| ACC-10 | Dirty worktree while cleanup requested | unique work preserved, actionable refusal |
| ACC-11 | Required check missing/unknown | no protected merge |
| ACC-12 | Stale criterion evidence | satisfaction downgraded/invalidated |
| ACC-13 | Work finished but deploy unknown | task maybe complete; deployed intent not satisfied |
| ACC-14 | Session lacking required intent binding | preflight rejects and explains source |
| ACC-15 | Two trusted peers cold start | discover and cooperate per policy without manual bootstrap |
| ACC-16 | Peer disconnect, crash, rejoin | no lost accepted events, stale lease fenced |
| ACC-17 | Unauthorized mesh/HTTP client | no mutation or credential exposure |
| ACC-18 | Fleet partial rollout | explicit partial, nonzero where contract requires, accurate hosts |
| ACC-19 | Mobile drawer, links, actions | keyboard/touch/focus working at 320-390 px |
| ACC-20 | Unsupported marketing promise | site claim/build gate fails with exact file+claim |
| ACC-21 | New typed entity | relevant CLI/API/MCP/UI/docs projections discover it automatically |
| ACC-22 | Project with only baseline .ai | init/update/doctor/start/check/finish usable without mesh/server |
| ACC-23 | Crash between merge and ledger update | reconciles to actual GitHub/commit evidence, no duplicate action |
| ACC-24 | Wrong binary SHA running | runtime proof says stale, not 'ready/current' |
| ACC-25 | Performance win | paired baseline/after with reproducible sampling & no gate weakening |

Every delivered behavior requires green positive + red negative/mutation test where meaningful. Mark unavailable test infrastructure as blocker or implement through existing harness. Attach actual run IDs, not narrative claims.
