+++
title = "A live scenario is not selected by a bare usecase run, its evidence is written under the ignored local half, and it never raises a command to covered; an undischarged obligation is reported unmet, never failed"
description = "A live scenario's result describes one tree, on one machine, at one minute. CI cannot"
weight = 141
[extra]
claim_id = "live-scenario-evidence-is-local"
status = "guaranteed"
source = "docs/claims/live-scenario-evidence-is-local.md"
+++
{% raw %}

## What it means

A live scenario's result describes one tree, on one machine, at one minute. CI cannot
reproduce it, so nothing downstream may treat it as reproducible evidence:

- `majordomus usecase run` with no arguments runs the fixtures and only the fixtures, and
  prints `live (run it with --live)` for the rest, so what CI means by a bare run does not
  change under a repository that adds a live scenario;
- `--live` adds them, and naming an id runs it whatever its mode;
- live evidence is written to `.ai/local/evidence/live/<id>.json`, which the local half
  ignores, rather than beside the committed fixture evidence under
  `.ai/local/evidence/use-cases/`;
- `usecase coverage` counts a live scenario as naming its commands and never as covering
  them, so a command only a live scenario exercises stays `partial`;
- an obligation the repository has not discharged is reported `unmet`, not `fail`.

That last distinction is the point of the separation: a failing fixture scenario is a
defect in the tool, and an unmet obligation is work not done. Reporting both in the same
word would make one of them unreadable.

## How it works

`mj_uc_selected` in `lib/usecase.sh` returns false for a live scenario unless it was named
or `--live` was given. `mj_uc_cmd_run` sets `MJ_UC_LIVE_EVIDENCE` to
`$MJ_AI_LOCAL_DIR/evidence/live`, runs the live scenarios one at a time after the fixtures,
and counts a live exit 10 into `unmet` rather than `failed`; the summary line and the JSON
document both carry the two counts separately. `mj_uc_coverage_rows` increments the
executable count only when the scenario is not live, so `mj_uc_status_of` can never reach
`covered` from live scenarios alone.

## How to see it

```bash
majordomus usecase run                     # live scenarios are listed, not run
majordomus usecase run --live              # exit 10 when an obligation is unmet
jq '.mode, .result' .ai/local/evidence/live/know-whether-this-work-is-finished.json
majordomus usecase coverage --json | jq '.rows[] | select(.status == "partial")'
bash test/run.sh 359_live_scenarios
```

`test/cases/359_live_scenarios.sh` proves each half: that a bare run writes no live
evidence, that `--live` writes it where git ignores it and leaves the tracked tree
untouched, that an unmet obligation is counted as `unmet` and not as `failed`, and that the
command the live scenario runs stays `partial` in coverage.
{% endraw %}
