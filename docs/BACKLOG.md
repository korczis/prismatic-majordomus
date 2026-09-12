# Backlog

The five quantities in this repository that grow on their own, the commands that measure
them, and the commands that reduce them. Behaviour as implemented and tested; where this
document and the scripts disagree, the document is wrong and changes in the same commit.
The rule is `project.accumulation-is-measured`; the gate is `scripts/ci/backlog-check`; the
behavioural cases are `test/cases/131_backlog_hygiene.sh` and
`test/cases/276_disk_is_bounded.sh`.

## The finding this exists because of

On 2026-09-11 the operator reported one symptom — *worktrees, processes and branches keep
piling up and nothing is being deployed* — and measurement found four separate quantities
at once (the fifth, disk, filled the volume the next day and has its own section below):

| Quantity | Measured | Of which |
|---|---|---|
| open pull requests | 16 | 15 merged cleanly on any developer's machine |
| linked worktrees | 64 | 47 with uncommitted work |
| local branches | 111 | 91 not merged into `origin/master` |
| running `majordomus` servers | 192 | 14 abandoned, one over a day old, 13 ports, ~150MB |

The deploy pipeline was not broken: `gh-pages` was one commit behind `master` and the site
was live. Nothing was failing. Every part was working as designed, and the result was still
a standstill — which is the point of this document.

## Why pull requests do not merge on GitHub, and never will

This repository commits its derived artifacts, and every one of them carries a fingerprint
of the tree it was generated from. After a merge that tree is neither parent's, so both
recorded fingerprints are wrong and no textual combination of them is right — the correct
value only exists after the generator runs. `.gitattributes` therefore marks those paths
`merge=derived` and `scripts/merge-derived` resolves them instead of conflicting.

A custom merge driver is an executable named by `git config merge.<name>.driver`. That
configuration is **per clone** and cannot be committed — which is why `just
derive-merge-driver` exists. GitHub's servers are a clone that has never run it. So GitHub
performs a plain three-way merge over files that every branch regenerates, finds conflicts
in all of them, and reports the pull request as `CONFLICTING` / `DIRTY`.

**GitHub's `CONFLICTING` is not a false positive.** The conflict is real for any clone that
lacks the driver, and GitHub is always such a clone. The same branch, measured both ways:

```sh
git                               merge-tree --write-tree origin/master <branch>  # exit 0
git -c merge.derived.driver=false merge-tree --write-tree origin/master <branch>  # exit 1, 15 conflicts
```

So there are two honest answers to "does this merge", and which one you get depends on
whether the driver is configured. The one that matters is the clone that will actually
carry the result — a developer's machine — and there the answer was yes for fifteen of the
sixteen open that day. The merge button is not slow or flaky; it is structurally
unavailable, and it will not become available. A contributor who waits for it waits
forever, and that wait is the whole backlog.

**Ask git for the exit code, never grep its output.** `merge-tree`'s conflict wording is not
a contract; a grep that misses it counts a conflicted merge as clean, which is the most
expensive way this measurement can be wrong. `scripts/land` uses the exit code.

## The commands

| Quantity | Measure | Reduce |
|---|---|---|
| pull requests | `scripts/land` | `scripts/land --run` |
| worktrees, branches | `majordomus worktree` | `majordomus worktree` cleanup, by hand for dirty ones |
| abandoned servers | `scripts/reap-orphans` | `scripts/reap-orphans --kill` |
| build output on disk | `scripts/reap-orphans --targets` | `scripts/reap-orphans --reclaim` |
| the whole gate | `scripts/ci/backlog-check` | `scripts/ci/backlog-check --remote` |

### `scripts/land`

Reads every open pull request, fetches its head, and asks git — not GitHub — whether it
merges. `--run` creates the integration branch's own worktree off `origin/master`, merges
the clean ones in order, and regenerates the derived artifacts, which are stale on a merge
result by construction because the driver resolved them to *ours*.

It stops before pushing, always, and prints the command that would publish. Landing is
reversible until the push; deploying is a decision, not a side effect. Use `--exclude` for
a pull request whose author is landing it themselves — check `majordomus_peers` first.

A pull request that merges cleanly against `origin/master` can still conflict against the
merges already on the integration branch. That is a real conflict, `land` reports it as
left behind, and it is resolved by hand on the branch.

### `scripts/reap-orphans`

A `majordomus serve` or `mcp` outlives its client whenever the client dies without closing
it: the parent is reaped, the server is re-parented to init, and it holds its port and its
memory until the machine restarts. A server that answers is not a server anyone is talking
to.

Sessions in this repository are each other's instruments, so **a process is never matched
by name**. `pkill -f majordomus` would end the servers that live peers are attached to. A
process is reaped only when all four hold, each re-checked immediately before the signal:

- it is a majordomus `serve` or `mcp` process, by executable path;
- its parent is init (`ppid 1`) — the session that started it is gone;
- it has zero `ESTABLISHED` TCP connections — no client is attached;
- it is older than `--min-age` (default 30m).

Dry-run by default; `--kill` acts. A server that acquires a client between the scan and the
signal is skipped.

### Build output, and the floor under a build

On 2026-09-12 this machine reached **zero bytes free** on a 926GiB volume. Every session on
it died in the same minute — not gracefully, but with the harness unable to write its own
task files, which reads to each session like broken tooling rather than like a full disk,
and `df` could not be run either. `~/dev/prismatic-majordomus-wt` held 180GB, of which
~145GB was `apps/majordomus-cli/target` in worktrees **whose branches were already merged**.
One had landed hours before and still held 32GiB. Nothing in finishing a piece of work
removes the build directory it produced, so nothing ever would have.

The same reaper answers it, with a second subject and a second consent (`--reclaim`, never
`--kill`). **It removes `target/` and never a worktree**: a false positive costs somebody a
rebuild and cannot cost a byte of source. A build directory is reclaimed only when all four
hold — merged into `origin/master`, nobody working in that worktree, `git status` empty, and
the directory provably cargo's own and inside that worktree. The primary checkout is
excluded by name rather than by predicate, because every other checkout borrows its
executable.

Liveness is read from the machine (`ps`, and every process's working directory) and never
from the peer board: an announcement belongs to a *connection*, and a worker that reconnects
keeps its work while losing its place on the board — exactly the window a reaper runs in.
Merged-ness alone would have been a disaster, and the instance was on the machine that
morning: a worktree holding 11.8GB with a live peer in it read as fully merged, because the
session working in it had not committed yet and its tip was still where the merge left it.
An mtime is never a predicate here (it is the one whose missing input becomes an extreme
value, and it cleaned three *active* worktrees once), and the reaper refuses as a whole —
reclaiming nothing, not everything — when liveness or `origin/master` cannot be read.

The other half is a bound rather than a warning. A `df` in `doctor` helps the next person
and cannot help the person already stuck. `mj_rust_space_check` in `lib/rust_bin.sh` refuses
to *start* a build below **5120MB** free, and the three paths that start one — the launcher,
`scripts/derive`, `just build` — all ask it. The floor is measured: a full `cargo build
--locked` of the crate into an empty `target/` wrote 2.45GiB, and the floor is two of those.
`MAJORDOMUS_MIN_FREE_MB=0` lifts it. A bound whose own input cannot be measured says so and
lets the build run — the deliberate opposite of the sweep, because refusing every build on a
machine whose `df` is unreadable stops all work to prevent a hypothetical.

### The derived trees that were never declared

`merge=derived` only helps a path that is marked. `scripts/generate-site-data` deletes and
regenerates fourteen whole sections under `site/content/` on every run — its `rm -rf
"$CONTENT_ROOT/..."` lines are the list — and on 2026-09-11 **thirteen of the fourteen were
not declared in `.gitattributes`**. Only `registry` was.

So every integration conflicted over `guarantees/`, `commands/`, `use-cases/`, `why/`,
`plan/`, `skills/` and the rest: content no branch owns, that the generator replaces
wholesale, and that a person then resolved by hand, per merge, forever. This was the single
largest mechanical cause of the backlog after the forge constraint itself, and it surfaced
here only because landing six pull requests hit it on the very first merge of `origin/master`.

All fourteen are declared now, and `scripts/ci/backlog-check` reads the list **out of the
generator** rather than keeping one by hand — a hand-kept list falls behind the first time a
section is added, which is exactly how thirteen omissions survived unnoticed.

### Why servers were abandoned in the first place

`majordomus serve --idle N` stops when no peer has been attached for `N` seconds, and
`DEFAULT_IDLE_SECONDS` (900) is the constant the shared server is started with —
`scripts/ci/entry-converges` holds that contract for the server `serve ensure` starts,
which is the path a person takes.

It is not the path that leaked. Raw `serve`'s `--idle` defaults to **0 — run until
stopped**, and four cases in the suite backgrounded a server without it. A case that is
interrupted before its cleanup trap fires — a killed run, a spend limit, a closed laptop —
leaves that server holding its port until the machine restarts. That is where the abandoned
servers came from, and it is why the same contract is now held for the suite's own spawns
by `scripts/ci/backlog-check` rather than only for the entry path. The two halves of one
rule, checked in one direction only, is the recurring shape here.

### Worktrees and branches

`majordomus worktree` derives the topology from git (`project.worktree-topology`) and marks
which branches are eligible for cleanup. The number eligible is normally far below the
number that look disposable: on the day above, 5 of 111. **Zero unique commits is not proof
that a branch is safe to delete** — a merged branch can hold uncommitted and untracked
authored work, and 47 of 64 worktrees were dirty. Classify the working tree, not only the
history, and push before removing anything.

## What the gate holds

A CI runner has no worktrees, no abandoned servers and no backlog of its own, so
`backlog-check` does not measure the quantities. It holds together the things whose absence
let them accumulate unseen: that each quantity still has its command, that the reaper is
safe by construction, that `.gitattributes` and its installer still agree — a declaration
checked in one direction only is how this class of defect survives — and that no document
sends a reader to the merge button. `--remote` adds the measurement itself, and fails when
more than `MJ_BACKLOG_MAX` (default 10) pull requests are landable and still open.
