+++
title = "The program that writes into the checkout says what it wrote, and whoever measures the tree reads that"
description = "The program that writes into the checkout says what it wrote, and whoever measures the tree reads that"
weight = 64
[extra]
kind = "rule"
slug = "project-a-producer-declares-what-it-writes-1"
identity = "project.a-producer-declares-what-it-writes@1"
status = "active"
source = ".ai/repo/rules/project/a-producer-declares-what-it-writes.v1.md"
+++
{% raw %}

## Rationale

CI records evidence with the tree it was measured in, and evidence from a dirty tree proves
nothing about the commit (ADR 0087). The rust job measured its tree after `scripts/rust-check`
ran, excluding the two outputs that run names: its timings file and its artifact directory.
The measuring step wrote both paths down itself, under the crate's directory, on the
reasoning that the script changes into the crate before it writes.

It does, for the artifact directory. The timings file it resolves earlier, against the
directory the caller stood in, so the file lands at the root of the checkout. The step
excluded a path where nothing was, counted the run's own timings as dirt, and every lane of
every run measured `dirty` — on master, on every pull request, for a week, with every job
green. The site could not publish any evidence as current, and the issue that asked for clean
measured trees could not close.

Nobody wrote anything careless. Two places each knew where one program writes, and one of
them was wrong about a line that had since moved. A list kept beside its reader goes stale
the same way a number kept beside its source does, which is the failure
`project.derived-once` names.

## Required behaviour

- A program that writes inside the checkout during a measured run declares those paths
  itself, at the point where it resolves them, relative to the root of the checkout, one per
  line. `scripts/rust-check` does so into the file `MJ_RUST_OUTPUTS` names, before anything
  else runs, so a run that fails has still said what it may have left behind.
- The declaration is written outside the checkout. A list inside the tree would be one more
  path to exclude.
- The step that measures the tree builds its exclusions from that declaration and from
  nothing else, and records the same paths as `excluded` in the measurement it hands on.
- A path the producer resolves outside the checkout is not declared: nothing there can make
  the tree dirty.
- A run that left no declaration excludes nothing. Whatever it wrote is then dirt, which is
  the conservative reading.

## Failure behaviour

`test/cases/357_ci_records_evidence.sh` fails when the measuring step of the rust job carries
an exclusion of its own, when a step that runs `scripts/rust-check` does not write the
declaration the measuring step reads, when the script names anything but its two outputs for
the workflow's own arguments, and when the step, run for real in a fixture checkout that
holds exactly those outputs, measures anything but `clean`. The same case requires that one
more file measures `dirty`, and that the list the step used to carry measures `dirty` — so
the case fails if the measurement stops distinguishing a run's own output from dirt.

What it does not catch, stated rather than implied: a producer that writes a path it does not
declare measures `dirty`, which is correct and is reported by the run, not by this case; and
the suite job still names its one output, the report, in the step that excludes it, because
the name is an environment value of that same job and resolves nowhere else.

## Verification

```sh
bash test/run.sh 357_ci_records_evidence
```

In a pull request's `validate` run, the `evidence` artifact's manifest carries
`"working_tree": "clean"` for the `crate` producer with `excluded` naming `timings.tsv` and
`apps/majordomus-cli/dist`.
{% endraw %}
