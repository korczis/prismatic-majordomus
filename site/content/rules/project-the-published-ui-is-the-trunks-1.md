+++
title = "The UI the site publishes is the trunk's, and a UI that is missing, older or cut short without saying so is a failed publication"
description = "The UI the site publishes is the trunk's, and a UI that is missing, older or cut short without saying so is a failed publication"
weight = 157
[extra]
kind = "rule"
slug = "project-the-published-ui-is-the-trunks-1"
identity = "project.the-published-ui-is-the-trunks@1"
status = "active"
source = ".ai/repo/rules/project/the-published-ui-is-the-trunks.v1.md"
+++
{% raw %}

## Rationale

ADR 0141 put the UI on the public site as a snapshot: `majordomus web report ui-snapshot`
reads the UI once at the commit being published and writes it as files, a job of the pages
workflow builds it, and the deploy job places it at the UI's mount. The ADR said what it left
out: no public probe of the mount after the deploy.

An audit on 2026-10-12 found what that omission amounted to. Every check of the publication
read something else. `scripts/pages verify` read `/build.json`, which names the site's commit
and the composed surfaces and not the snapshot. `verify-docs` and `verify-rustdoc` read their
own surfaces. `scripts/ci/pages-check`, the `pages-live` gate that runs at every `finish` and
on a thirty-minute schedule, asked five questions and none about the UI. And the fact that a
publication holds the snapshot at all was written in one place, a step of
`.github/workflows/pages.yml`, so the other deploy path did not know it: `scripts/site-deploy`
run by a person empties the published branch and copies `site/public`, which the site build
never puts the snapshot into, and so published a site whose UI mount answered 404 with every
gate green.

Three failures followed from one cause, a surface that exists only where its producer ran and
that nothing downstream was held to:

- **Missing.** A deploy that did not run the workflow's step drops the mount.
- **Older.** A snapshot left in `site/public` from an earlier build, or a CDN still answering
  the previous files, serves a UI read at another commit beside pages that name the new one.
  The UI is the product's first screen; a reader has no way to see that it is last week's.
- **Short.** The producer is bounded on purpose and records what it left out and what stopped
  it. A tree half copied, or a record that does not say what ended the reading, looks the
  same as a whole one unless the pages the record lists are asked for.

A relation checked at one stage and owned by nobody at the next is this repository's most
repeated defect (`project.the-crate-reference-is-published` is the same finding about the
crate's reference). The stage after the producer is owned now.

## Required behaviour

1. **Declared once.** `.ai/repo/ci/pages.yaml` names, under `deploy.snapshots`, each surface a
   publication holds that the site build does not compose: the served surface it is a
   snapshot of, where its producer writes it, the producer, and the attribute its pages state
   their commit with. The mount is not written there; it is the topology's mount of that
   surface (`docs/generated/web.json`), because a publication holds the snapshot where a
   process serves the original. `scripts/pages snapshots` is the one reading of the entry, and
   the workflow, the deploy, the probe and the gate each go through it.
2. **Read at the commit that is published.** The snapshot a deploy publishes names, in its
   `surface.json`, the commit the site's pages name. The trunk's head, within the deploy
   window, is what the site is held to (`scripts/ci/pages-check`, sections 1 and 3); the UI is
   held to the site's commit and therefore to the same window, never to a looser one of its
   own.
3. **No deploy path without it.** `scripts/site-deploy` refuses a site whose tree lacks a
   declared snapshot at its mount, carries one read at another commit than `HEAD`, or carries
   a declaration that names another mount than the one it stands at. The refusal names the
   producer and the placement. `--release-metadata` is not a deploy of the site — it adds two
   files to what the branch already serves — and is not asked.
4. **Verified where a reader meets it.** `scripts/pages verify-ui` asks the public site: the
   snapshot's `surface.json` answers, names the commit and the mount it is served at; the
   landing page answers and its banner states the same commit; `snapshot.json` answers, names
   the same commit, and says what ended the reading and which pages were left out; and every
   page that record lists answers 200. The pages asked are bounded by `--max-pages`, spread
   evenly over the list, and the verdict states how many were asked, how many were not, how
   many the snapshot itself left out and what stopped it.
5. **Cut short is stated, never hidden.** A snapshot that stopped at its limit or its budget
   is a pass that says so. One whose record does not say what ended the reading, or does not
   list what it left out, is a failure: "every page" and "the pages there was time for" must
   be distinguishable from the public bytes alone.
6. **A gate, not a note.** The pages workflow runs the probe after a deploy, once the site
   serves the commit, and the step is not `continue-on-error`. `scripts/ci/pages-check` asks
   it of the published commit as section 2c; a UI that is absent, another commit's or not
   whole is exit `10`, and one that could not be judged is exit `12` with the half named in
   the verdict line, never a pass.

## Failure behaviour

- `scripts/pages snapshots` exits `12` when a declared surface has no mount in the topology
  or an entry does not name its artifact, producer and marker.
- `scripts/site-deploy` exits `10` with `REFUSE` for each case of item 3, and for a model that
  is present and cannot be read; nothing is pushed.
- `scripts/pages verify-ui` exits `10` when the site answered and the UI is absent, another
  commit's or not whole — one `FAIL` line per finding on stderr, each ending with the command
  that shows it again — and `12` when the site never answered or no commit could be read to
  hold the UI to.
- In `.github/workflows/pages.yml` it is the step `the UI it publishes`; a failure fails the
  deployment. In `scripts/ci/pages-check` it is section 2c of the `pages-live` gate, which
  `finish` asks (`at-finish`) and the scheduled pages run asks every thirty minutes, deploying
  when it fails.

## Verification

`test/cases/1096_the_published_ui_is_the_trunks.sh`, with no Rust executable and no network:
the model's entry resolves its mount from the topology and the workflow builds and uploads
what it says; `verify-ui` against a site served locally passes a snapshot read at the commit,
with and without `--commit`, and refuses by name a stale `built_from`, a missing
`surface.json`, a listed page that is absent, a record of another run, a banner of another
commit and a record that does not say what ended its reading, states what a cut-short
snapshot left out and what `--max-pages` did not ask, and returns 12 for a site that does not
answer; `pages-check` passes the UI read at the published commit and exits 10 for another
commit's and for none; `scripts/site-deploy` refuses a site without the snapshot, with
another commit's and with a misplaced one, and publishes `HEAD`'s. `test/cases/180` and
`test/cases/96` hold the gate and the deploy over a whole site that carries one;
`test/cases/1092` and `test/cases/1093` hold the producer.
{% endraw %}
