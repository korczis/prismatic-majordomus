# Deploy the site by hand

When the operator wants the site live now, not after the Actions queue. One command, the
same one the Pages workflow runs, and the deploy is visible: the gh-pages commit and the
site's footer both name the source commit.

## When

The operator says so, in words, in this session. A merge to master already deploys through
the workflow; the hand deploy is for the cases where that is too slow, or the workflow is
red for a reason that is not the site (a benchmark runner, a macOS job) and the site itself
is green. Never deploy to make a red gate look green.

## Preconditions

- You stand on the commit to publish, with a clean tree, and `origin/master` contains it.
  A preview of unmerged work is possible with `--any-ref`; the footer then names that
  commit, so nobody mistakes it for master. Say so when you do it.
- `zola`, `node_modules` (`npm ci`) and, for the full gate, `cargo` are present; the script
  says what it skips.
- GitHub Pages serves the `gh-pages` branch. Once per repository:
  `scripts/site-deploy --configure-pages` (needs `gh`); after that the setting stays.

## Do

```bash
scripts/site-deploy --dry-run      # gate, build, check; shows the gh-pages commit it would push
scripts/site-deploy                # the same, then pushes gh-pages
scripts/site-deploy --probe        # add the browser probe (every route at 320, 390, 1280 px)
```

`just site-deploy` is the same command. The gate is: derived data in sync, generated
projections in sync (`majordomus generate --check`), `site-build`, `site-check`. A failure
anywhere stops before the push, with the reason; fix the source, never the output.

## Verify

GitHub serves the new branch head in about a minute. The deploy is real when the live site
names the commit you published:

```bash
curl -s https://korczis.github.io/prismatic-majordomus/ | grep -c "/commit/$(git rev-parse HEAD)"   # 1
```

Then open the page you changed. Record the deploy where the task keeps its notes
(`majordomus checkpoint`) with the source commit.

## Do not

- Edit anything under `site/public/` or on `gh-pages` by hand: both are outputs. A wrong
  page is fixed in its source and redeployed.
- Deploy from a dirty tree with `--allow-dirty` to "just see it": the footer would name a
  commit that does not contain what is live.
- Bypass a refused gate. `REFUSE` lines are the reasons the deploy would mislead; the script
  exits 10 and pushes nothing.
