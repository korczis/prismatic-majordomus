# The website and every committed derived artifact. `derive` regenerates them in
# dependency order; `derive-check` refuses a stale one.

# Regenerate site/data/generated and the derived docs from README, docs/, the policy skeleton and CLAIMS.yaml.
[group('site')]
site-data:
    scripts/generate-site-data

# Exit 10 when the derived site data is stale.
[group('site')]
site-data-check:
    scripts/generate-site-data --check

# Deploy the site by hand: gate, build, check, push gh-pages (see .ai/repo/skills/deploy-site/SKILL.md). `just site-deploy --dry-run` shows what it would push.
[group('site')]
site-deploy *args:
    scripts/site-deploy {{args}}

# Build the website (zola).
[group('site')]
site-build:
    scripts/site-build

# Build the documentation source for the running executable: mounted at /docs, written into target/web/docs with the surface.json that makes it discoverable — one source, one generator, two base URLs, and `just serve` answers /docs/ from it.
[group('site')]
site-docs:
    scripts/site-build --serve

# The static checks over the built site (scripts/site-check).
[group('site')]
site-check:
    scripts/site-check

# Every route at three widths in a real browser, SITE_PROBE_JOBS routes at a time (default 4 here); `just site-probe --quick` samples one route per template.
[group('site')]
site-probe *args:
    SITE_PROBE_JOBS="${SITE_PROBE_JOBS:-4}" scripts/site-probe {{args}}

# Serve the website locally in watch mode.
[group('site')]
site-serve:
    scripts/site-serve

# The publication fast path, exactly as .github/workflows/pages.yml runs it: prove the committed derived data current by its input hash, render it, check the output. `just pages build`, `pages check`, `pages paths`, `pages budget`, `pages current`, `pages verify`.
[group('site')]
pages *args:
    scripts/pages {{args}}

# The controlled publication path, measured N times; `just pages-benchmark -n 10 --write-baseline` records this platform's baseline under .ai/repo/benchmarks/pages/.
[group('site')]
pages-benchmark *args:
    scripts/pages benchmark {{args}}

# Every committed derived artifact, regenerated in dependency order (scripts/derive: generate, site data, generate again over the index the site data changed).
[group('site')]
derive:
    scripts/derive

# Exit 10 naming every stale derived artifact — the registry's projections and the site's data — and write nothing (scripts/derive-check).
[group('site')]
derive-check:
    scripts/derive-check

# Declare the `derived` merge driver this clone needs, so .gitattributes resolves the derived artifacts on merge instead of conflicting on their fingerprints (scripts/merge-derived).
[group('site')]
derive-merge-driver:
    git config merge.derived.name "derived artifacts: resolve to ours, regenerate before committing"
    git config merge.derived.driver "{{root}}/scripts/merge-derived %O %A %B %P"
    @echo "merge.derived wired; .gitattributes now resolves the derived artifacts on merge"

# Take the conflict off a branch GitHub reports as CONFLICTING over derived files alone: merge master in where the driver exists, derive, commit, push the branch (scripts/unblock). `just unblock 137`, `just unblock feature/x --dry-run`.
[group('site')]
unblock *args:
    scripts/unblock {{args}}
