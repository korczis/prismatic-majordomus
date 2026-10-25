---
schema: skill/v1
id: deploy-site
version: 1
title: Deploy the site by hand
description: Publish the website to GitHub Pages from a terminal, through the one script the Pages workflow runs, when waiting for the Actions queue is not acceptable.
status: active
tags: [site, deploy, evidence]
related: [repo-review]
inputs:
  - the commit to publish, checked out with a clean tree and contained in origin/master
  - zola, node_modules (npm ci) and, for the full gate, cargo; or, without cargo, the rustdoc, ui-snapshot and cockpit-snapshot artifacts of the same commit's Pages run
  - the operator's word, in this session, that the site is to go live now
outputs:
  - a gh-pages commit naming the source commit, or a refusal naming the gate that stopped it
  - a checkpoint recording the deploy and the commit it published
---

# Purpose

Put the site live now, not after the Actions queue, without inventing a second way to
publish. One command, the same one the Pages workflow runs, and the deploy is visible
afterwards: the `gh-pages` commit and the site's footer both name the source commit.

# When to use

The operator says so, in words, in this session. A merge to master already deploys through
the workflow; the hand deploy is for the cases where that is too slow, or the workflow is
red for a reason that is not the site (a benchmark runner, a macOS job) and the site itself
is green. Never deploy to make a red gate look green.

# Procedure

## 1. Preconditions

- You stand on the commit to publish, with a clean tree, and `origin/master` contains it.
  A preview of unmerged work is possible with `--any-ref`; the footer then names that
  commit, so nobody mistakes it for master. Say so when you do it.
- `zola`, `node_modules` (`npm ci`) and, for the full gate, `cargo` are present; the script
  says what it skips.
- GitHub Pages serves the `gh-pages` branch. Once per repository:
  `scripts/site-deploy --configure-pages` (needs `gh`); after that the setting stays.

## 2. Deploy

```bash
scripts/site-deploy --dry-run      # gate, build, check; shows the gh-pages commit it would push
scripts/site-deploy                # the same, then pushes gh-pages
scripts/site-deploy --probe        # add the browser probe (every route at 320, 390, 1280 px)
```

`just site-deploy` is the same command. The gate is: derived data in sync, generated
projections in sync (`majordomus generate --check`), `site-build`, `site-check`. A failure
anywhere stops before the push, with the reason; fix the source, never the output.

The site build does not compose the UI's snapshot (ADR 0141): the pages workflow writes it in a
job of its own and its deploy job places it. A deploy from a terminal has no such job, so the
script refuses a built site that lacks a snapshot the publication model declares
(`scripts/pages snapshots` lists them), or carries one read at another commit, and prints the
commands that place it (`project.the-published-ui-is-the-trunks`). After that refusal the site
is already built and checked, so write the snapshot at this commit, copy it to its mount and
publish what is there:

```bash
majordomus web report ui-snapshot
mkdir -p site/public/ui && cp -R target/web/ui-snapshot/. site/public/ui/
scripts/site-deploy --skip-build
```

## 3. Without cargo: the artifacts of the commit's Pages run

The parts of a deploy that need cargo are the crate's reference and the two snapshots, and
the Pages run of the commit already built them as the artifacts `rustdoc`, `ui-snapshot` and
`cockpit-snapshot`. When that run produced them but did not publish (a step bound killed its
deploy job, or the queue is too slow), do the deploy job's steps by hand with them instead of
building from source. The operator's word is still the precondition, and so are these:

- A fresh clone at the commit, with full history and tags, because the link check decides
  commit and tag links (`scripts/pages checkout --verify` says whether it can), and `npm ci`
  in that clone: a `node_modules` borrowed from another checkout can lack a package the build
  loads.
- A case-sensitive volume for the clone and for `TMPDIR`, while the UI snapshot holds two
  paths that differ only in case (I3447): on the default case-insensitive APFS of macOS the
  artifact does not extract whole. A sparse image formatted `Case-sensitive APFS` with
  `hdiutil create` serves.

```bash
sha="$(git rev-parse HEAD)"
run="$(gh run list --workflow pages.yml --commit "$sha" --limit 1 \
  --json databaseId --jq '.[0].databaseId')"
for a in rustdoc ui-snapshot cockpit-snapshot; do
  gh run download "$run" -n "$a" -D "target/web/$a"
done
npm ci
scripts/pages evidence --commit "$sha"     # site/data/evidence.json, as the deploy job writes it
scripts/pages build                        # reads target/web/rustdoc; no generation, no cargo
scripts/pages check
scripts/pages snapshots | while IFS="$(printf '\t')" read -r id mount artifact _; do
  [ -n "$id" ] || continue
  mkdir -p "site/public$mount" && cp -R "$artifact/." "site/public$mount/"
done
scripts/site-deploy --skip-build
```

Order matters: the reference is in place before the build reads it, and the snapshots are
placed after the check, which judges the site's own pages. `scripts/site-deploy` still
refuses a snapshot read at another commit, so an artifact of an older run cannot go live
under this one's name. Verify the UI and the Cockpit as well as the site (step 4).

## 4. Verify

GitHub serves the new branch head in about a minute. The deploy is real when the live site
names the commit you published:

```bash
curl -s https://majordomus.dev/ | grep -c "/commit/$(git rev-parse HEAD)"   # 1
```

`scripts/pages verify-ui --commit "$(git rev-parse HEAD)"` asks the same of the UI at `/ui/`:
its snapshot names that commit and every page it lists answers;
`scripts/pages verify-ui --surface cockpit --commit "$(git rev-parse HEAD)"` asks it of the
Cockpit.

Then open the page you changed.

To read it the other way — you are looking at the live site and want to know what it is —
`git log -1 --format='%an %s' origin/gh-pages` names the source commit and says who
published it: `github-actions[bot]` for a push to master, a person for a hand deploy. A
preview published with `--any-ref` stays live until the next push to master publishes over
it, so a live site that is not master is expected rather than wrong.

## 5. Do not

- Edit anything under `site/public/` or on `gh-pages` by hand: both are outputs. A wrong
  page is fixed in its source and redeployed.
- Deploy from a dirty tree with `--allow-dirty` to "just see it": the footer would name a
  commit that does not contain what is live.
- Bypass a refused gate. `REFUSE` lines are the reasons the deploy would mislead; the script
  exits 10 and pushes nothing.

# Output

The commit that is now live, named, and the branch it was pushed to; or, when the gate
refused, the `REFUSE` line that stopped it and the source that has to be fixed. Record the
deploy where the task keeps its notes (`majordomus checkpoint`) with the source commit, so
that what is live is answerable later without asking GitHub.
