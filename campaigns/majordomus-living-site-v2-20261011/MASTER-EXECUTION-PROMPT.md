# Mission: Make majordomus.dev a verifiably living, derived product surface

You are the implementation agent, not an analyst providing a future wish-list. Current target: `github.com/korczis/prismatic-majordomus` and `https://majordomus.dev/` especially `/roadmap/`.

## Known starting evidence, not timeless assumptions
- `site/content/roadmap.md` is authored front matter + prose and names `.majordomus/project/milestones/`.
- `site/templates/roadmap.html` consumes generated roadmap/project/capabilities/roadmap-graph JSON; text names `.ai/repo/project/milestones/`.
- `scripts/generate-site-data` uses transactional staging and provides `--check` and `--out`; `scripts/derive` stages A-D with a lock.
- `docs/PLAN_STATUS.md` is generated from `.ai/repo/project/`.
- Previous deployed roadmap appeared behind released versions; verify live HEAD and deployment, do not treat these observations as permanent.
- CI plans through `.ai/repo/ci/gates.yaml` and `.github/workflows/validate.yml`, release via `.github/workflows/release.yml`.

## Success contract
A site page is current only relative to a SPECIFIED target: deployed release/source SHA, generated inputs and generated tool version. The website must never implicitly claim to represent today's master if it is a pinned release snapshot. Show data provenance and staleness where valuable. A deployment MUST refuse mixed revisions, unproven claims and stale generated dependencies. Static authored explanations are reviewed and linked to canonical facts; historical docs are labeled as historical, not automatically rewritten.

## Execution
1. Read README, `manifest.json`, TRACKING and M00–M07 prompts.
2. Inspect live repository / PRs / workflows / deployment. Reconcile work with existing intent/milestone/issues. Do not duplicate.
3. Run M00 and publish baseline evidence, then execute milestones in dependency order, parallelizing truly independent issues using available authorized mesh workers.
4. Every accepted increment: test -> derive/check -> merge -> analyze version -> release if due -> deploy approved surface -> smoke verify -> record evidence -> continue.
5. Update .ai rules/doctrines/gates AND existing generator tooling, public docs, runtime API/MCP/UI projections from canonical data as appropriate.
6. Do not ask for "proceed using majordomus mesh". Maintain durable handover; a process/supervisor must actually persist or state why unattended continuation cannot work.
7. Stop only at fully verified acceptance, explicit authority/budget limit or genuine external blocker. Never silently downgrade controls to get green.

## Final evidence report
List baseline and final revisions, every PR, which issues merged, changed contracts, tests + results, CI durations and cache, generated outputs, release version/tag, deploy URL and inspected live SHA, remaining blocks and rollback plan. If any step was not performed, say NOT DONE.
