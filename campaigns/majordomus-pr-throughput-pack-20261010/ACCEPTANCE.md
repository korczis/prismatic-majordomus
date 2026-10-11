# Proof matrix (all required before claiming operational success)

| Layer | Positive proof | Negative/race proof | Surface |
|---|---|---|---|
| Queue snapshot | one typed inventory/next action | unknown GitHub result is not empty | CLI JSON, API, MCP, UI |
| Mergeability | local derived driver correct | stale SHA / GitHub conflict mismatch | prs explain |
| DAG | stack root→child flow | cycles, vanished parent, draft | CLI graph, Cockpit |
| Policy | authorized safe action | missing owner approval / protected branch | .ai validator, CI |
| Drain | repeated progress, reobserve each step | concurrent actors / crash / external master push | CLI, workflow |
| CI | required statuses on exact ref | old green, missing status, timeouts | CI plan/verdict |
| Cleanup | terminal disposable tree removed | dirty worktree untouched | worktree reconcile |
| Workflow | event trigger acts without storm | fork privilege escalation denied | GitHub Actions |
| Sessions | issue/intent/session traced | new WIP budget exceeded | preflight/finish |
| Projections | parity from one fixture | missing record/offline/race, mobile a11y | Cockpit/UI/API/MCP |
| Release | analyzed/versioned tag, signed assets | duplicate tag; bad checksum | release/installer |
| Pages | source commit published, docs/demo works | stale pages fails live proof | Pages |
| Fleet | authorized converged version/mesh | unreachable node partial, safe rollback | fleet/doctor |

## Nonnegotiable gating
- Unit tests, property/state transition tests, real git fixtures, integration GitHub sandbox tests when available, script/CI validation, schema checks, full derivation drift checks, UI browser tests, security/effect policy validation, docs example tests.
- All gates use actual existing canonical commands discovered at HEAD. No guessing `just` aliases. Maintain cost/performance timings and compare before/after with sample size, no fabricated percent improvement.
- Provide a red-before/green-after regression for each significant bug where feasible.
- First show demo in controlled environment; real master and fleet only under authorized existing policy.
- Report exact merged SHA, version tag, released assets, Pages served SHA, fleet node verdict. If any absent, not 'done'.
