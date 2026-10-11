# Majordomus Public Surface: Atomic Autonomous Execution Pack

Target: latest `korczis/prismatic-majordomus`, with `master` as observed default publication branch. The checkout and canonical `.ai` are authoritative; never blindly trust a prompt's assumptions about filesystem, release policy, or runtime.

## Real repository anchors verified during pack preparation

- `site/data/homepage.toml` owns the ordered homepage chapters (`hero`, `recognise`, `map`, `how`, `refuses`, `surfaces`, `proof`, `install`) and asset budgets (`assets_bytes = 340000`, `inline_json_bytes = 8000`). `scripts/ci/homepage-check` enforces bidirectional agreement.
- `site/data/marketing.toml` owns editorial positioning, not generated counts, capabilities or versions; `scripts/site-check` enforces its line budget and rejects numeric claims.
- `docs/PRODUCT.md`, `.ai/repo/features/`, `apps/majordomus-cli/src/product.rs`, `site/data/registry/product.json` express the one-feature/one-model projection, enforced via `project.product-surface-derived` and ADR 0023.
- `scripts/generate-site-data`, `scripts/site-build`, `scripts/site-check`, `scripts/pages` are existing generation / publication entry points. `scripts/pages` advertises `build`, `check`, `fingerprint`, `current`, `paths`, `budget`, `benchmark`, `built`, `rebuild`, `verify`, and `verify-docs` subcommands.
- `.github/workflows/pages.yml` is a fast-path adapter over `scripts/pages` and uses GitHub Pages with `gh-pages`. `.ai/repo/ci/pages.yaml` is the model cited in that workflow. Existing scheduled publication repair must be preserved.
- Never presume a Pages deployment can be complete on merge alone: confirm the externally served commit and smoke routes with canonical verification.

## Definition of 'atomic'

Each prompt is one reviewable, testable, releaseable vertical change with explicit sources, files to *discover*, validation, and output artifacts. Split further if necessary, not into layers that cannot deploy. Every completed slice produces: evidence → canonical gates → release decision → push/merge using repo workflow → Pages publication proof → updated state/progress ledger. For a slice that has no public-output change, record N/A for publication rather than manufacturing a dummy Pages build.

## Truth/safety constraints

- Work through Majordomus native intent/milestone/issues/session/mesh tooling *if actually present*. Do not fabricate commands.
- A passing unit test is not a proof of published Pages. A successful Pages push is not a proof of externally visible version.
- An absent GitHub permission, missing secret, unavailable remote, failed gate, or external outage must become a **typed blocker with exact evidence**. Never mark green, never bypass gates or branch protection, and continue only safe independent work.
- Version bumps are determined by existing Elm-like / semantic / release doctrine: *do not presume every atom merits minor*. Batch atoms when policy does not permit individual release; record why.
- Update `.ai` after discovering a general reusable invariant, not by adding a cosmetic rule per UI widget. Prefer existing validators and executable gates; no decorative governance.
- No synthetic statistics, screenshots represented as live proof, fabricated demos, duplicate inventories, or hard-coded versions.
- No React/SPAs, backend rewrite or wholesale stack migration; use existing Zola/Tailwind/Flowbite/Alpine/Cytoscape/ECharts selectively when supported and justified.

## The release loop, applied after each eligible atom

1. Rebase/sync with canonical branch, inspect outstanding work and worktrees; avoid overlapping file ownership and racey changes.
2. Record intent criterion, impacted canonical entities, and proof plan.
3. Implement smallest vertical slice; add negative, positive, and cross-surface tests.
4. Regenerate once via canonical tooling; validate `scripts/pages current`, site build/check, appropriate `.ai`/schema/design/CI tests. Rebuild is not needed for purely non-site changes unless gates require it.
5. Re-run the same generation/check sequence and confirm idempotence; inspect diff for generated drift or private data leakage.
6. Use current version/release decision policy; bump only when required by it; generate release notes/provenance when appropriate. Re-run gates after bump.
7. Commit, PR, review, merge, push using existing repo controls. Never force-push protected branches or circumvent checks.
8. If public-affecting: use canonical Pages path and `scripts/pages verify --url <canonical-public-url> --commit <deployed-sha> --smoke` plus `scripts/pages verify-docs ...` as applicable. Confirm deployment `built` state. Record delayed/unavailable state as pending, not success. If existing retry/remedy mechanism is appropriate, use it without duplicate triggering.
9. Update milestone criterion, issue, evidence ledger, session handover, docs and `.ai` only as justified by generalized invariant. Publish changelog/status summary.
10. Start next independent atom. A failed deployment does not justify rolling additional unverified public changes blindly into an unknown state.

Do not retry infinitely: bounded attempts, retain exact exit codes and proof. Keep prompt-file names and atom IDs in your progress reports.
