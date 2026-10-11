# RELEASE 15 · 01 The site serves a release before its record lands

## Operating contract
As in 00. This milestone is in flight: issue I2303, ADR 0131 (proposed), branch
`feature/the-release-publishes-its-metadata`, owned by session p5 (2028245f) until it says
otherwise on the board. Read the branch, its PR and the board before touching anything.

## Objective
majordomus.dev serves `releases/<tag>.json` and `releases/latest.json` minutes after the archives
are published, without waiting for the record's pull request, and never serves an older release
than it served.

## Issue 1: Land I2303
`scripts/site-deploy --release-metadata <tag>` overlays exactly those two files on gh-pages;
an ordinary deploy keeps a newer served release; release.yml's publish job runs the mode after
`scripts/derive-check`; smoke waits for the served metadata. Cases 1041 and 87.
Acceptance: merged through `majordomus prs`; the first release after it shows latest.json served
before the record PR merged (prompt 00's timeline proves it).

## Issue 2: The dispatch of pages on master stays harmless
The publish job still dispatches `pages.yml` on master. Prove on the first real release that the
deploy it starts keeps the newer metadata (the `kept releases/latest.json` line in its log) and that
`scripts/pages verify` and the `pages-live` gate stay green while the record is a proposal.
Acceptance: run ids and log lines in the completion report.

## Issue 3: A record that never lands is visible
If the record's PR is closed unmerged, the site serves a release master does not record. Make
`release-check` (and the doctor) name that state with the tag, the PR and its age.
Acceptance: case with a served-but-unrecorded release.

## Completion report
PR numbers, merge shas, the first release's measured gap between "served" and "record merged".
