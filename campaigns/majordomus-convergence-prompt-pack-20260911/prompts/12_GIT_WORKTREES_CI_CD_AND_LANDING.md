# Stage 12 — Worktrees, CI/CD and Landing

Finish the repository hygiene that repeatedly leaves implemented work invisible or unlanded.

## Tasks

1. Enumerate all git worktrees, branches, staged/unstaged changes, stashes and local divergence.
2. Identify work relevant to the convergence that is not on the target branch.
3. Compare semantically before merging. Do not blindly merge stale/duplicate implementations that conflict with the new canonical architecture.
4. Port/cherry-pick/reimplement only the needed behavior and tests.
5. Remove obsolete worktrees/branches according to repository policy once safely integrated.
6. Run full canonical local quality pipeline, including:
   - formatting/lint,
   - Rust/shell tests,
   - coverage gates,
   - rules/doctrine doctor,
   - canonical ordering,
   - development semantics,
   - lease/parser single-source,
   - generated drift/docs/site,
   - API/OpenAPI/MCP/Cockpit contract tests,
   - security/secret checks,
   - distribution/install checks where applicable.
7. Review complete diff and commit in coherent units.
8. Push to the correct remote/branch.
9. Observe actual CI results; fix failures, repeat until green.
10. Verify generated site/deployment and runtime deployment if pipeline includes them.
11. Ensure branch protection/gates include the new checks so future code cannot bypass them.

## Prevention

If the project has repeatedly accumulated unlanded work, implement the canonical detection/prevention:

- doctor/Cockpit status for dirty/unmerged worktrees,
- session finish invariant that reports uncommitted/unpushed/unintegrated work,
- CI/project-plan linkage where appropriate,
- explicit evidence state for landed/deployed/verified.

Do not make destructive git cleanup automatic without safe confirmation and provenance.

## Acceptance

- Target remote branch contains all required convergence work.
- CI is green at the landed SHA.
- No required implementation is stranded in a worktree.
- Docs/site/deploy status is verified or an external access blocker is explicitly documented.
- Prevention tooling catches a deliberately created stale/unintegrated fixture/state where practical.
