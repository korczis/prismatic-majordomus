# Every finding a command reports names the command that reproduces it

## What it means

A finding without a way to see it again is an opinion. The reproduce command is what lets
the next person, or the next session, check the fact rather than trust the report — and in a
repository worked on by several agents at once, trusting a report nobody can re-run is how a
wrong finding survives for a week.

## How it works

`mj_finding` in `lib/common.sh` takes the reproduce command as its fifth argument, and
prints the finding without it when it is absent. That is the defect: a caller that forgot
one produced a finding that reads *exactly* like a finding that could not have one. Nothing
could tell them apart, so nothing did, and a reviewer was the only thing between the two.

`scripts/ci/finding-reproduce-check` reads every call to a reporting helper in the shell
tool and the scripts — `mj_fail`, `mj_drift`, `mj_warn`, `mj_doctrine_fail` — and counts its
arguments. Three decisions make the count honest:

- **A helper named inside a string is prose about the helper, not a call to it.** The scan
  walks each line outside its quoted runs; without that, this check would report itself.
- **A forwarding call** — `mj_fail "$@"` — passes whatever it was given, so the count belongs
  to whoever called it, and that caller is measured where it is written.
- **A call the gate cannot parse** is reported as `unreadable` and counted, never silently
  passed. An unparsed call that read as compliant would be this very defect, one level up.

`mj_ok` and `mj_info` are statements rather than findings and are not measured: there is
nothing to reproduce about a line saying a thing is fine.

## How to see it

```
scripts/ci/finding-reproduce-check            report, and fail on new debt
scripts/ci/finding-reproduce-check --strict   fail on any finding, baseline ignored
```

## What it does not cover

Whether the command actually reproduces the finding. The gate counts arguments; it cannot
run what it reads. A caller passing `"true"` as its fifth argument satisfies it, and that is
review's to catch.

It also measures only this repository's shell tool and scripts. Findings the Rust executable
reports carry their reproduce command through a typed field rather than a positional
argument, so they cannot have this defect; the behavioural cases are not measured either,
because a case asserts on findings rather than reporting them.

## Why it exists

Because the two states were indistinguishable in the output. `mj_finding` prints the finding
without a reproduce command when none is passed, so "this finding has no command" and "this
finding could not have one" rendered identically, and a reader had no way to tell a caller's
omission from a genuine absence.

## What proves it

The check itself, run against this repository in CI on every change. It landed with an empty
baseline, which is the part worth stating: it found nine violations across `lib/doctor.sh`
and `lib/watch.sh`, and those were fixed rather than recorded, so the gate holds a line at
zero instead of holding one at nine.
