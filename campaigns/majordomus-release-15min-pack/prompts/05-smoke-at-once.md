# RELEASE 15 · 05 Smoke runs the moment the metadata is served

## Operating contract
As in 00.

## Objective
The six smoke jobs install the release with the published installer within 1 minute of the site
serving it, and the release is not called done before all six are green. On v0.19.1 the smoke
jobs were created 354-369 minutes after the run started; on v0.20.0 smoke was skipped because the
publish job failed.

## Issue 1: Smoke waits for the served metadata, and only for it
With ADR 0131 the wait is for `releases/latest.json` naming the tag (prompt 01 changes the step).
Bound the wait by the budget (2 minutes for Pages plus margin), not by 15 minutes, and report the
measured wait into prompt 00's timeline.
Acceptance: case 87 holds the bound; the timeline records the wait.

## Issue 2: A skipped smoke is not a passed release
When publish fails, smoke is skipped and the run is red, but nothing re-runs smoke after a manual
recovery (v0.20.0: nobody did). The release capability (prompt 07) or the executor reruns the
smoke jobs once the metadata is served, and the release's state says "smoke not run" until then.
Acceptance: case: publish failed, metadata served by hand, smoke rerun and recorded.

## Issue 3: macOS smoke runners
Two of six smoke targets run on macOS runners. Measure their queue; if over budget, decide
(with evidence) between pre-warmed self-hosted macOS (the cluster has machines) and GitHub-hosted.
Acceptance: measurement and decision.

## Completion report
Smoke start and finish relative to "served", per target.
