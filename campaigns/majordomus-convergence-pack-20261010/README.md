# Majordomus: Verified Delivery Convergence

**Audit date:** 2026-10-10. **Target:** `korczis/prismatic-majordomus`, branch `master` and all open PRs.

## Evidence and limits

Inspected publicly available master repository (GitHub shows 3,438 commits and 52 PRs at inspection), recent merge commits c8f0e4c5 (#855) and a804ae20 (#874), related commit messages, published integration and guarantees documentation, and an older local context archive (dated 2026-09-11). The archive is historical, NOT current proof. The live per-PR list, check runs and branch protection configuration could not be completely fetched. **Therefore 52 is a navigation count, not a validated PR-by-PR assessment.** The published PR integration snapshot (2026-10-06) says 16 open PRs at that moment; it must not be treated as current.

Notable facts: `prs refresh/status/drain` already have classification, deterministic ranking, SHA-based relation-to-master, lease, verification and replanning after each merge (ADR 0101). Latest work fixes one-derive-at-a-time and orphan publication (#855). Release archive provenance was added in #874 (v0.20.0). Derive was observed to take 699s on Linux and over an hour on a loaded Mac in f1a892eb. Published CI guarantee reported `master` unprotected on Sep 11; **must re-check now**. Existing `.ai/repo/ci/gates.yaml`, completion, evidence, doctor, rules, generated docs and capability projections are integration targets, not concepts to rebuild.

## Intended sequence

Run `00` first. It produces a measured audit and a current PR graph. Run `01`-`06` in order, choosing only work actually needed after `00`. Each numbered file is one candidate milestone; each `## Issue` heading is one candidate issue. Work in small scope-limited PRs. Use the existing Majordomus issue/milestone lifecycle, PR integration and GitHub synchronization discovered at runtime. **Do not create a second orchestration system.**

Files:

- `prompts/00-current-reality-and-pr-triage.md`: precise live audit; mandatory baseline.
- `prompts/01-canonical-execution-evidence.md`: one evidence/trace model, eliminate duplicate state.
- `prompts/02-incremental-derived-graph.md`: bounded derive, reproducible outputs, cache and concurrency.
- `prompts/03-pr-integration-throughput.md`: repair stale PR backlog with existing integration engine.
- `prompts/04-policy-gates-and-completion.md`: policy-backed non-bypassable validation and deploy evidence.
- `prompts/05-derived-ui-mcp-observability.md`: expose canonical execution/PR state via existing projections.
- `prompts/06-self-hosted-proof-release.md`: end-to-end proof in this repo and a disposable external repo.

## Universal completion contract

No fake green. Each milestone must supply code, tests (happy/failure/concurrency where relevant), canonical docs, claims/evidence, appropriate machine-readable enforcement, CI wiring, verified commands, regressions and clear before/after measurements. Preserve cross-platform portable shell and Rust conventions. Never invent interfaces, API endpoints, crate names, commands, counts or performance results: inspect and adapt. Never claim GitHub/CI protection without querying it. No forced merge, bypassed gate, self-upgrading production or unsafe destructive cleanup. Explicitly record external blockers.

## Sources

- https://github.com/korczis/prismatic-majordomus
- https://github.com/korczis/prismatic-majordomus/pull/855
- https://github.com/korczis/prismatic-majordomus/pull/874
- https://github.com/korczis/prismatic-majordomus/commit/f1a892ebc66d585ec789605c3f082d7830c7ec44
- https://majordomus.dev/docs/integration/
- https://majordomus.dev/docs/integration-rollout/
- https://majordomus.dev/guarantees/ci-verdict-is-required/
