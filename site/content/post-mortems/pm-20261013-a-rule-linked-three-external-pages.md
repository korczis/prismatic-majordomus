+++
title = "A rule linked three external pages and the offline link check refused the site"
description = "A rule linked three external pages and the offline link check refused the site"
weight = 1
[extra]
kind = "post-mortem"
slug = "pm-20261013-a-rule-linked-three-external-pages"
identity = "pm-20261013-a-rule-linked-three-external-pages"
status = "resolved"
source = ".ai/repo/post-mortems/pm-20261013-a-rule-linked-three-external-pages.md"
+++
{% raw %}

## A rule linked three external pages and the offline link check refused the site

### What failed

Three consecutive `pages` runs on the trunk ended in the `check` step of `deploy` with

    FAIL link refused /rules/project-ssh-key-access-is-preauthorized-1/: https://code.claude.com/docs/en/permissions: code.claude.com cannot be decided offline

and the same line for `geminicli.com` and `learn.chatgpt.com`, then `this run ended (publish:
skipped) without pushing gh-pages; the site still serves an older commit`. In `validate`
38364727993 the same three lines failed the `site` job and, in shard 1, cases
`12_site_build` and `489_rustdoc_deploy`, which build the site. The `pages` job of that run
then failed on the consequence: `the site owes a publication: 62d6ca2e45f4 changed what it
is built from 4105s ago`. Production served `73d3c8cc31` from 06:59Z until the fix was
deployed at 10:15Z.

### Root cause

The rule's Rationale cited the three vendors' documentation as Markdown links. A rule is
published as a page, so the links became links of the site, and `scripts/ci/link-check`
decides every link without the network: a host it has no declared answer for is refused,
not skipped. Whether the site was built with the rule in it before the landing was not
established for this record; the first run that refused it is the trunk's.

### Fix

`b7ef193abe` (pull request 1079) writes the three addresses as code spans. The rule still
names its sources and the site carries no link it cannot decide.

### Prevention

`test/cases/12_site_build.sh` fails on this locally in the time it takes to build the site;
it was not run before the merge. The pages gate judging a commit while that commit's own
deploy is in flight is issue I2640.
{% endraw %}
