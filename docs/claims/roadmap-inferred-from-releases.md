# The roadmap places every live milestone in the horizon its derived status gives it, attributes shipped work to the published release that shipped it, and reports where the records and the released commits disagree

## What it means

The roadmap is not written down. Every milestone that has not been cancelled or superseded
appears in one of four horizons, chosen by its derived status alone. A milestone is *now* when
work has started (`ACTIVE`, `VERIFY`), *next* when nothing it requires is unfinished
(`PLANNED`), *later* when something it requires is not done (`BLOCKED`), and *done* when it is
`DONE`.

No milestone carries a version. A version is a published release. The roadmap says which
issues each release completed, by the issue's `completed_at`, and which ones its commits named,
by the references the changelog resolves.

Four signals mark places where two committed facts disagree:

- `shipped_unrecorded`: a released commit names an issue whose record reads not started.
- `awaits_evidence`: every issue is done and the milestone's own evidence is missing.
- `no_issues`: no issue names the milestone.
- `idle`: work has started, and nothing was completed or named in the last five releases.

## How it works

`roadmap::infer` in `apps/majordomus-cli/src/roadmap.rs` takes three inputs: the derived plan,
the changelog's release sections and the intents. It returns the horizons, the releases, a pace
and an optional forecast per milestone, and the signals.

The function has two consumers:

- `plan.roadmap` answers with it as `inferred`, including the work since the last release.
- `majordomus generate` writes it to `site/data/registry/roadmap.json` from published releases
  only, and `/roadmap/` renders that file.

`apps/majordomus-cli/tests/roadmap.rs` asks the built executable over MCP and checks two
things: the horizons, and the intent the fixture's milestone realises. The unit tests in
`roadmap.rs` cover attribution, pace, the forecast and each signal.

## How to see it

```bash
curl -s http://127.0.0.1:<port>/api/v1/plan/roadmap | jq '.inferred.horizons[] | {horizon, milestones: [.milestones[].id]}'
jq '.signals, .releases[0]' site/data/registry/roadmap.json
```

## What it does not cover

A signal is a report, not a transition. It never moves a record. The issue is transitioned by
its worker, and a milestone is accepted by a person.

The forecast is a count of releases at a measured pace, not a version number. Releases are cut
on the measured contract, so the version of the release after next cannot be named in advance.

The open pull requests are not read: the forge's state is not a committed fact.

## Why it exists

The roadmap it replaced showed eight of twenty-six milestones, under versions typed into their
records (`0.5` twice). The tool had shipped `0.18.0`. Records that lagged the released code were
invisible. A roadmap that is inferred cannot fall that far out of date, because it has nothing of
its own to update.
