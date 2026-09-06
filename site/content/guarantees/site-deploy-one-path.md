+++
title = "The site is deployed by one script, scripts/site-deploy, from a terminal or from the publication workflow; it refuses a dirty tree, a commit master does not contain and a build that is not HEAD's, pushes site/public to gh-pages with the source commit named, and pushes nothing when the output is unchanged"
description = "scripts/site-deploy is the only way the site reaches GitHub Pages. The Pages workflow runs it after its gate; a person runs it when the operator wants the site live without waiting for the Actions queue (.ai/repo/skills/deploy-site/SKILL.md, just site-deploy). Either way the same checks run and the same branch is pushed: gh-pages, which GitHub Pages serves."
weight = 127
[extra]
claim_id = "site-deploy-one-path"
status = "guaranteed"
source = "docs/claims/site-deploy-one-path.md"
+++
{% raw %}

## What it means

`scripts/site-deploy` is the only way the site reaches GitHub Pages. The Pages workflow runs it after its gate; a person runs it when the operator wants the site live without waiting for the Actions queue (`.ai/repo/skills/deploy-site/SKILL.md`, `just site-deploy`). Either way the same checks run and the same branch is pushed: `gh-pages`, which GitHub Pages serves.

## How it works

The script refuses (exit 10, nothing pushed) a working tree with uncommitted changes, a HEAD that `origin/master` does not contain (`--any-ref` publishes a preview and says so), and a `site/public` whose footer does not name HEAD. Unless `--skip-build`, it runs the gate: `scripts/generate-site-data --check`, `majordomus generate --check` when cargo is present, `scripts/site-build`, `scripts/site-check`, and the browser probe with `--probe`. It then copies `site/public` into a worktree of `gh-pages` (created orphan when the branch does not exist), adds `.nojekyll`, commits with the source commit and the registry fingerprint in the message, and pushes. Unchanged output is not committed again; `--dry-run` shows the commit it would push. `--configure-pages` points GitHub Pages at the branch once, through `gh`.

## How to see it

```bash
scripts/site-deploy --dry-run       # gate, build, check; "would push <sha> to origin/gh-pages: deploy: site from <source>"
scripts/site-deploy                 # the same, then the push; GitHub serves it in about a minute
curl -s https://korczis.github.io/prismatic-majordomus/ | grep -c "/commit/$(git rev-parse HEAD)"   # 1
```

## What it does not cover

The script does not decide when to deploy; the operator does, and the workflow does on every push to master. It does not make a red gate green: a failed check stops before the push. It does not serve the site itself; GitHub's own "pages build and deployment" does, and that step is GitHub's.

## Why it exists

Two deploy paths, one for CI and one for a person, would be two places for the gate to drift and no way to tell from `gh-pages` where a page came from. `test/cases/96_site_deploy.sh` clones the checkout, gives it a local bare remote, builds the site, and proves every refusal pushes nothing, the first deploy creates `gh-pages` with the site, `.nojekyll` and the source commit in the message, the redeploy of unchanged output pushes nothing, and a `site/public` built from another commit is refused.
{% endraw %}
