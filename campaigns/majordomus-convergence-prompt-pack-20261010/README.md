# Majordomus Convergence / Delivery Acceleration Pack

**Target:** `github.com/korczis/prismatic-majordomus`  
**Prepared:** 2026-10-10  
**Primary consumer:** Claude Code Opus (or comparable agent with repository, GitHub and local execution access)  
**Intent:** close the gap between declared architectural guarantees and actually verified, merged, released behavior.

## Audit-derived leads (verify fresh at execution)
At inspection on 2026-10-10, default branch `master` was at `c8f0e4c5017cd2aefa434a12c6fa95c615a3f9a8`; public releases listed `v0.19.1` while a `0.20.0` bump was merged. GitHub showed 52 open PRs (42 not drafts); recent successful validate workflows took approximately 59–98 minutes. The repository model included `apps/majordomus-cli/src/capability/builtin/`, `apps/majordomus-cli/src/mesh/`, `.ai/repo/ci/gates.yaml`, `scripts/ci-plan`, `docs/CI.md`, `docs/MESH.md`, `.github/workflows/{validate,queue,release,pages,version}.yml`.

**Critical PR leads** (might already have moved): mesh durability #894, pagination #893, trust revocation #892, bounded mesh inputs #891, MCP retry safety #888, MCP transport bounds #889, slow HTTP clients #898, retired runtime #897, lease/serve #883–#887, worktree reconcile #882, CI durations/shards #881 and #880, intent opposition #807, satisfaction #813, Cockpit intent #819, completion #292, external release #872, fleet #900. These are not merge instructions; inspect conflict/readiness/dependencies individually.

## Execution
1. Feed `00-ORCHESTRATOR.md` to Claude Code at repo root. It delegates the remaining prompts in dependency order.
2. If autonomous agent orchestration is unavailable, feed each `stages/NN-*.md` sequentially, starting with `01` and using the preceding report as context. The stages are independently runnable after refreshing repo state.
3. Require evidence in `.ai/` using the repository's current practices; do not create a second private bookkeeping system.
4. Run `checks/ACCEPTANCE-MATRIX.md` before claiming the mission completed. A feature is delivered only when correct behavior is present on an integrated, evidenced revision.

## Design constraints
- No new framework, UI or shell pipeline where canonical typed capabilities suffice.
- No feature proliferation while correctness/merge queues are blocked.
- User-facing behavior must be derived and explainable, including `unknown` and freshness, never mocked or hardcoded.
- Protect existing user changes/worktrees. Never equate a passing PR check on an old SHA with merge-ready evidence.
- GitHub Actions duration is end-to-end elapsed time, not simply CPU time. Measure queue, runner startup, dependency fetch, build, actual tests, artifact handoff, critical path, verdict separately.
- Rollout should be observable and reversible. Do not sacrifice mesh/security checks to hit latency metrics.

## Success metrics (targets, not current facts)
- fast structural verdict p95 <= 60 seconds; targeted merge validation p95 <= 5 minutes where safe, with risk-based mandatory correctness gates;
- critical correctness PRs merged or explicitly blocked by actionable independent evidence;
- worktree cleanup provably nondestructive and reconciled with Git/GitHub state;
- release archive provenance verified against commit, version and supported platform, smoke-tested from a foreign repo;
- a real end-to-end intent → opposition → plan → implementation → evidence → completion → projection demonstration;
- clean mobile UI, no production mock state, health `unknown` not healthy, freshness visible;
- measured decrease in PR age, integration lead time, flaky retries and derived-state churn.

## Files in this pack
- `00-ORCHESTRATOR.md`: autonomous mission control and stop rules.
- `stages/01-BASELINE-AND-QUEUE.md`: inventory, PR graph, WIP/backpressure.
- `stages/02-RUNTIME-MESH-MCP.md`: correctness, trust, durability and lifecycle.
- `stages/03-CI-CRITICAL-PATH.md`: measuring and reducing actual CI latency.
- `stages/04-WORKTREES-AND-INTEGRATION.md`: safe incremental merges and cleanup.
- `stages/05-RELEASE-DISTRIBUTION.md`: provenance, foreign checkout, rollback.
- `stages/06-INTENT-CLOSURE.md`: working intent loop, not paperwork.
- `stages/07-UI-PROJECTIONS.md`: mobile-first true-state Cockpit and CLI/MCP consistency.
- `stages/08-DOGFOOD-AND-RATCHET.md`: self-hosted acceptance, ongoing enforcement.
- `checks/ACCEPTANCE-MATRIX.md`: measurable proofs and stop criteria.
