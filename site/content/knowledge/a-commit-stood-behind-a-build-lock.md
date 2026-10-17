+++
title = "A commit stood fourteen minutes behind another worktree's build lock, in silence"
description = "A commit stood fourteen minutes behind another worktree's build lock, in silence"
weight = 1
[extra]
kind = "knowledge"
slug = "a-commit-stood-behind-a-build-lock"
identity = "a-commit-stood-behind-a-build-lock"
status = "verified"
source = ".ai/repo/knowledge/curated/a-commit-stood-behind-a-build-lock.md"
+++
{% raw %}

## A commit stood fourteen minutes behind another worktree's build lock

### What was observed

2026-10-13, one shared machine, load average between 70 and 140, nineteen cargo processes
of other worktrees. A `git commit` in a feature worktree ran the pre-commit hook, the hook
ran `bin/majordomus-cli`, and the launcher printed `building the executable (debug
profile) ...` and then nothing. Ten minutes in, the foreground command reached its
timeout and went to the background; at fourteen minutes the cargo process was inspected: 0%
CPU, no child process, and the lock file under `debug/` of the shared build directory held
open. It was not building. It was waiting for a lock.

The same afternoon, measured by another session with `ps` and `lsof` at 13:56Z: five cargo
processes in four other worktrees at 0.0% CPU for between 20 and 42 minutes — `cargo test`
and `cargo build` of four fixes whose executables share one target directory — while one
`rustc` ran on the whole machine. The fourteen minutes above were the short instance.

### The cause, in the order it was found

1. `.cargo/config.toml` names one `build-dir` for every worktree (ADR 0139). Cargo takes an
   exclusive lock on a build directory, so every build in every worktree is in one queue.
2. ADR 0139 named that cost and accepted it because `derive` runs one at a time. The other
   builders were not counted: the commit hook's launcher, `rust-check`, the documentation
   build, the cases that run `cargo test`.
3. Cargo says "Blocking waiting for file lock on build directory" as a status line.
   `--quiet` removes status lines, and every caller passed `--quiet`. The wait was silent
   by construction.
4. Nothing bounded it. `project.every-wait-is-bounded` asks a timeout, a completion
   predicate and a recovery path of every wait; `scripts/liveness-check` enforced it for
   network calls and background processes and read no build.
5. The worker's part: the note "export a private CARGO_BUILD_BUILD_DIR per worktree"
   already existed in the session's own memory and was not applied, and the silence was
   read as a slow build for fourteen minutes before the process was looked at.

### What was changed

- `lib/rust_bin.sh`: `mj_cargo_bounded` runs cargo without `--quiet`, keeps what it says,
  counts while cargo is blocked, and at `MAJORDOMUS_BUILD_LOCK_WAIT` seconds (default 60)
  says so and builds in the tree's own directory; `mj_cargo_private_dir` names it.
- `bin/majordomus-cli`, `scripts/derive`, `derive-check`, `rust-check`, `cockpit-probe`,
  `release-package`, `site-deploy` and `scripts/ci/entity-check` build through it.
- `scripts/liveness-check` has a fourth shape, `unbounded-build`; four test cases that
  still call cargo directly are recorded as debt in the baseline.
- Rule `project.a-build-never-waits-in-silence-on-anothers-lock`, ADR 0139 amended,
  `docs/WORKTREES.md`, case 3002.

### What would have caught it earlier

- Reading the accepted cost of a decision as a claim with a scope: "derive already runs one
  at a time" was true and was about one caller of many.
- Inspecting silence at the interval expected of the work. A warm build of this crate takes
  under a minute; a launcher that says nothing for two has already told you something.
- A gate for every shape of a rule's subject, not only the shapes that had bitten before.
  The same shape exists wherever several writers share one output directory; the adoption
  sessions' shared target directory, measured the same afternoon, is the next place to look.
{% endraw %}
