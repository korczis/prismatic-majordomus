# A live scenario is not selected by a bare usecase run, its evidence is written under the ignored local half, and it never raises a command to covered

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

## What it does not cover

It keeps a live run's evidence out of the tracked tree; it does not make that evidence
durable. `.ai/local/evidence/live/` is this checkout's, so another worktree, another
machine and a fresh clone each start with none of it, and nothing replicates it. Evidence
that must survive a checkout is a ledger execution, which is what `evidence record` writes.

It bounds where the answer is written, not how long it is true. A live answer describes the
repository at the moment it was asked; nothing here expires it or notices that the tree has
moved since. Staleness is the evidence subsystem's question, and `proof-is-an-execution`
is where it is answered.

And it does not make a live scenario count. A live run stays `partial` in coverage by
construction, so a command whose only scenario is live is reported as not fully covered —
that is the intended reading, not a gap this claim closes.

## Why it exists

A use case that asks its questions of the repository it was invoked in produces an answer
about somebody's work in progress, and the first place such an answer would have landed was
the repository's own published evidence — where it would have been read as a property of the
project rather than of one machine at one minute. Writing it under `.ai/local/`, which the
layer already declares is never published, keeps the live answer available to the person who
asked and invisible to everyone reading the repository.
