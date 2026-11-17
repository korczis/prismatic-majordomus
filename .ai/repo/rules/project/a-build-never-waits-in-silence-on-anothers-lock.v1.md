---
id: project.a-build-never-waits-in-silence-on-anothers-lock
version: 1
kind: rule
title: A build never waits in silence behind another build's lock
description: Every build of the executable that this repository's own tooling starts goes through one bounded launcher; a build that stands behind another worktree's lock on the shared build directory says so, stops waiting at a declared bound, and builds in its own directory instead.
statement: A build started by the repository's tooling runs through lib/rust_bin.sh's mj_cargo_bounded; behind a lock it waits no longer than the declared bound, says what it waited behind, and then builds in the tree's own build directory or stops with a reason.
status: active
class: blocking
depends_on: [project.every-wait-is-bounded@2, project.worktrees-share-build-intermediates@1]
tags: [process, shell, build, worktrees]

x-majordomus:
  tests: [test/cases/3002_a_build_never_waits_in_silence.sh, test/cases/122_liveness_gate.sh]
---

# Rationale

The worktrees of this repository share one build directory for what cargo compiles on the
way (ADR 0139), and cargo takes one exclusive lock on a build directory. The decision named
that cost in one sentence — two worktrees building at once wait for each other — and set it
aside because `derive` already runs one at a time. The builds that are not `derive` were not
counted: the commit hook's launcher, the lints, the tests and the documentation build each
start cargo too, in every worktree, whenever a worker reaches for them.

On 2026-10-13, with nineteen cargo processes on the machine, a commit hook ran
`cargo build --quiet` and stood behind that lock for fourteen minutes: no CPU, no child
process, no line of output. Cargo does say that it is blocked, as a status line, and
`--quiet` removes status lines; every caller in the repository passed `--quiet`. The worker
read the silence as a slow build, which is the reading `project.every-wait-is-bounded`
exists to refuse, and the rule did not reach it, because its scan looked for network calls
and background processes and no build.

A wait behind a lock is the plainest wait there is — nothing is being computed, and the
waiter can know that from the first second. It had no bound, no message and no way out.

# Required behaviour

Every build, test run, lint or documentation build of the executable that a tracked script
of this repository starts is started through `mj_cargo_bounded` in `lib/rust_bin.sh`, or in
a file that makes the word `cargo` mean it.

That launcher keeps what cargo says, so that the line about the lock exists to be read
whatever the caller asked to see. While the last thing cargo said is that it is blocked on
a build directory, the launcher counts; at `MAJORDOMUS_BUILD_LOCK_WAIT` seconds (sixty,
unless set; zero never waits) it says how long it waited and which other cargo processes
exist, stops that cargo, and runs the same build in the tree's own build directory, after
the disk floor has been asked. A build that is already in a directory of its own and is
still blocked at the bound stops with exit 75 and says what holds it.

A build that got the lock before the bound is not moved, and a build nobody is in the way
of runs exactly as before, in the shared directory, and says nothing the caller did not
ask for.

# Failure behaviour

`scripts/liveness-check` reports a direct `cargo build`, `test`, `clippy`, `doc`, `check`,
`run` or `rustc` at command position in a tracked script as an `unbounded-build` finding,
and the `structure` job fails on one that `.ai/repo/liveness-baseline.txt` does not record.
A call through `mj_cargo_bounded` is not a finding, and neither is a file whose `cargo` is a
function that calls it. A workflow is not read for this shape: a runner has one checkout and
no other build to stand behind. The baseline names the test cases that still call cargo
directly; each is debt, and removing a line is done by routing that case through the
launcher.

What the scan cannot see stays with the worker: a `cargo` typed by hand, or started by a
tool that is not a tracked script of this repository, takes the same lock with nothing to
bound it. Several worktrees given one `CARGO_TARGET_DIR` or one build directory by hand are
one queue; a worker that does so gives each its own, or runs them one at a time, and
inspects a build that has said nothing for two minutes — its CPU, its children and the lock
file it holds open — instead of waiting on it.

# Verification

`test/cases/3002_a_build_never_waits_in_silence.sh` drives the launcher with a stand-in
cargo: a free build runs once in the shared directory and prints nothing; a build behind a
lock is bounded, says so and is run again in the tree's own directory; a lock that came
free in time moves nothing; a blocked build with nowhere else to go stops with 75; the disk
floor is asked before a private build; a failing build shows its error although the caller
asked for quiet; the gate reports a direct call, accepts a bounded one and does not read a
sentence as a call; and the launcher the commit hook runs builds through the function.
`test/cases/122_liveness_gate.sh` holds the gate green on the checkout it ships on.
