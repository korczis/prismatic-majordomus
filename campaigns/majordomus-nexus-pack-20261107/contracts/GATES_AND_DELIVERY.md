# Gates, safety, integration and release contract

## Preflight checks

- Verify repo/HEAD, clean worktree, remotes, policy, current CI, GitHub blockers, active peers and canonical worktree topology.
- Discover actual command names; never treat copied examples as authoritative.
- Check collisions before new paths or IDs, and announce work/scope via mesh.
- Inspect secrets exposure, filesystem scope, prompt archive privacy and plugin permissions.
- Establish expected branch / PR / release lifecycle. Pin current base SHA to make rebase decisions reproducible.

## Developer loop

1. Inspect exact code owner and write a bounded acceptance test.
2. Make minimal canonical implementation; remove obsolete duplicate code if safely possible.
3. Run focused tests, formatting, schema checks, security and performance checks where relevant.
4. Run project-level checks, read **diagnostic contents**, not merely exit code.
5. Update actual `.ai` source only when justified; regenerate derived artifacts and prove zero drift.
6. Maintain durable task checkpoint and source-linked evidence; stage/push verified commits early.
7. Use only established merge/rebase/release flows; batch coherent prompts into few PRs.

## Non-bypassable conditions

- No `--no-verify`, blanket test deletion, fabricated checks, false `verified` status or pretending a queued CI run passed.
- No `--force` to protected master/trunk and no bypass of required reviews.
- No secrets in commit, model-to-model prompts, public UI or published site.
- No unbounded autonomous loops, arbitrary provider spend, automatic external provisioning, secret/key management, or unaudited irreversible changes.
- Fail-open only for explicitly classified **read-only prompt capture availability**, with loud diagnostic; fail-closed for authorization of dangerous effects. Validate which hook events actually support a deny decision.

## Release lane definition

A lane is an integration unit; an individual prompt is an atomic delivery checkpoint, not necessarily a PR. Preferred maximum 3–5 cohesive PRs across the entire Nexus implementation, subject to actual merge policy and team concurrency. Release version bump according to repository policy (patch/minor); avoid fictitious bumps on every micro-step. To say *deployed*, prove new trunk SHA and relevant deployment probe.

## Completion evidence

For each prompt: changed files and canonical owner, tests (exact command/result), proofs (fixture, integration log or URL), docs/.ai updates, performance deltas where measured, introduced/deleted duplicates, merged/blocked status. Never use fake screenshots, static UI placeholders or hypothetical metrics.
