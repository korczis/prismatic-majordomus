---
schema: post-mortem/v1
id: pm-20261013-a-rule-linked-three-external-pages
kind: post-mortem
title: A rule linked three external pages and the offline link check refused the site
description: A link to a host the link check cannot decide offline stops the site build, the pages deploy and every case that builds the site; cite an external page as a code span.
status: resolved
date: 2026-10-13
runs:
  - workflow: pages
    id: "38359869013"
    commit: fe56c1b0da
    jobs: ["deploy"]
    step: "check"
  - workflow: pages
    id: "38364727988"
    commit: ba3fd916b3
    jobs: ["deploy"]
    step: "check"
  - workflow: pages
    id: "38373835702"
    commit: ba3fd916b3
    jobs: ["deploy"]
    step: "check"
  - workflow: validate
    id: "38364727993"
    commit: ba3fd916b3
    jobs: ["site", "suite-shard (1)", "pages"]
cause:
  class: defect
  summary: Rule project.ssh-key-access-is-preauthorized linked three vendor documentation pages as Markdown links; its published page then carried three links the offline link check refuses, so the site was not built and not deployed.
components:
  - file:.ai/repo/rules/project/ssh-key-access-is-preauthorized.v1.md
  - file:scripts/ci/link-check
  - file:.github/workflows/pages.yml
  - file:scripts/ci/pages-check
signals:
  - id: refused-link
    text: "cannot be decided offline"
  - id: rule-page
    text: "/rules/project-ssh-key-access-is-preauthorized-1/"
  - id: unpublished
    text: "without pushing gh-pages; the site still serves an older commit"
  - id: owed-publication
    text: "the site owes a publication"
fixed_by: ["commit:b7ef193abe", "pr:1079"]
prevention: ["test:test/cases/12_site_build.sh"]
tags: [pages, site, links]
related: ["rule:project.ssh-key-access-is-preauthorized@1", "issue:I2647"]
---

# A rule linked three external pages and the offline link check refused the site

## What failed

Three consecutive `pages` runs on the trunk ended in the `check` step of `deploy` with

    FAIL link refused /rules/project-ssh-key-access-is-preauthorized-1/: https://code.claude.com/docs/en/permissions: code.claude.com cannot be decided offline

and the same line for `geminicli.com` and `learn.chatgpt.com`, then `this run ended (publish:
skipped) without pushing gh-pages; the site still serves an older commit`. In `validate`
38364727993 the same three lines failed the `site` job and, in shard 1, cases
`12_site_build` and `489_rustdoc_deploy`, which build the site. The `pages` job of that run
then failed on the consequence: `the site owes a publication: 62d6ca2e45f4 changed what it
is built from 4105s ago`. Production served `73d3c8cc31` from 06:59Z until the fix was
deployed at 10:15Z.

## Root cause

The rule's Rationale cited the three vendors' documentation as Markdown links. A rule is
published as a page, so the links became links of the site, and `scripts/ci/link-check`
decides every link without the network: a host it has no declared answer for is refused,
not skipped. Whether the site was built with the rule in it before the landing was not
established for this record; the first run that refused it is the trunk's.

## Fix

`b7ef193abe` (pull request 1079) writes the three addresses as code spans. The rule still
names its sources and the site carries no link it cannot decide.

## Prevention

`test/cases/12_site_build.sh` fails on this locally in the time it takes to build the site;
it was not run before the merge. The pages gate judging a commit while that commit's own
deploy is in flight is issue I2640.
