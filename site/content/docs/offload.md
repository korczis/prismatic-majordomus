+++
title = "The derive offload"
description = "the derive offload: a branch's derive run on a declared cluster machine in its turn — the commands and exit codes, the `derive-offload` declaration, host choice by free space and queue length, the FIFO queue with dead-waiter removal, the bundle round trip with its refusals and a build directory of each derive's own, and the board and mesh etiquette"
weight = 57
[extra]
source = "docs/OFFLOAD.md"
+++

{% raw %}

A derive regenerates every committed derived artifact, and every session needs one before
it can commit. It takes about ten minutes on an idle machine and up to an hour on the shared
MacBook under load. `majordomus offload` runs a branch's derive on another machine the
repository declares, in that machine's turn, and brings the derived files back staged.
Nothing is committed or pushed. Behaviour as implemented and tested. Where this page and the
executable disagree, the page is wrong and changes in the same commit.

The module is [`apps/majordomus-cli/src/offload/`](https://github.com/korczis/prismatic-majordomus/tree/@source-ref@/apps/majordomus-cli/src/offload). The
rule is `project.a-derive-offload-waits-its-turn`, and the issue that made it is I2398.
Before it, the offload was a script in a home directory with its hosts typed into it. Its
lock was `until mkdir lock; do sleep 15; done`. On 2026-10-11 at 00:18Z five sessions waited
on lundra that way, the oldest for 2 h 12 min, while later arrivals won the lock.

## The commands

```sh
majordomus offload hosts                  # the declaration, defaults filled in; contacts nobody
majordomus offload probe                  # measure every host, name the one a derive would take
majordomus offload derive .               # derive this worktree on the chosen host, stage the result
majordomus offload derive . --host lundra # on a named host (a declared id or address)
majordomus offload derive . --keep        # keep the host's worktree afterwards
```

`--declaration <file>` reads a declaration file instead of the repository's, and
`--format json` prints one document. The executable examples are in the generated reference
([`generated/cli.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/docs/generated/cli.md), under `majordomus offload`).

<div class="overflow-x-auto" tabindex="0">

| exit | `offload derive` |
|---|---|
| 0 | derived; the changed files are staged, or the derive changed nothing |
| 2 | an untracked file, an unresolved merge, or a merge with unstaged changes: nothing travelled |
| 10 | the derive failed on the host; the last lines of its log are printed and its worktree is kept there |
| 11 | the derive deleted a tracked file; nothing was applied, and the snapshot is named for applying by hand |
| 12 | no declared host can take it, the host could not be used, or the result did not apply |
| 75 | the queue's declared wait ran out; the ticket is withdrawn |

</div>


`offload probe` exits 12 when no host is eligible. `offload hosts` exits 0 even when nothing
is declared: an empty declaration is an answer.

## The declaration

The hosts and their limits are written once, in
[`.ai/repo/offload/derive.yaml`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/offload/derive.yaml). The kind is
`derive-offload` and the schema is `derive-offload/v1`
([`share/schemas/majordomus/derive-offload/`](https://github.com/korczis/prismatic-majordomus/tree/@source-ref@/share/schemas/majordomus/derive-offload)).
Every index build validates it, and it is served like any other object of the layer, as
`majordomus://derive-offload/derive`. No script and no line of the crate carries a host
address.

<div class="overflow-x-auto" tabindex="0">

| field | holds | default |
|---|---|---|
| `hosts[]` | `id`, `address` (`user@host`), optional `identity_file`, `min_free_gib`, `description`; the order breaks ties | — (one is required) |
| `min_free_gib` | GiB that must be free under a host's home for it to be chosen | 10 |
| `base` | the ref the bundle is cut against | `origin/master` |
| `queue.directory`, `queue.lock` | the ticket directory and the lock, absolute on the host | `/tmp/offload-queue`, `/tmp/offload-derive.lock` |
| `queue.poll_seconds`, `queue.wait_max_seconds` | between looks, and in all | 5, 14400 |
| `remote.checkout`, `remote.worktrees` | the host's checkout and where each derive's worktree is made, under its home | `dev/prismatic-majordomus`, `dev/prismatic-majordomus-wt` |
| `remote.target` | the shared build directory each derive clones its own from | `.cache/majordomus-offload-target` |
| `remote.spool` | where the bundles and the derive's log are written | `/tmp` |
| `remote.path`, `remote.derive`, `remote.touch` | the `PATH` prefix, the derive command, and the files touched before it | asdf, cargo and Homebrew bins; `scripts/derive`; the crate roots |

</div>


Adding a machine is one entry under `hosts:`. The machine needs ssh access without a prompt,
a checkout of this repository at `remote.checkout` whose `origin` it can fetch, and the
toolchain `scripts/derive` builds with. `offload probe` is the check that it answers.

## Choosing a host

Every declared host is asked once, over ssh, for the free space under its home and for its
queue. A host is skipped, and the probe says why, when it:

- does not answer, or answers something the queue does not read, such as a login banner;
- has less free space than its minimum;
- reports no free-space figure, because unknown is never enough.

Of the hosts left, the derive takes the one with the fewest derives running or waiting:
every live ticket, plus one when the lock is held. A tie goes to the host declared first.

## The queue

Each host runs one offloaded derive at a time, and its waiters are served in the order they
arrived. The decision is made in Rust over what the host reports (`offload/queue.rs`). The
host runs short scripts that only observe and act. They take every value as an argument,
never as part of their text.

- A waiter starts a **holder** on the host, a process that lives exactly as long as the
  waiter's connection. It then writes a ticket named `<arrival>-<holder pid>-<tag>`, where
  the arrival is nineteen digits of nanoseconds since the epoch.
- The waiter takes the lock only when its ticket is the oldest one whose holder is alive.
  The lock is a directory made by `mkdir`, and it holds one empty file per process of the
  derive that holds it, named by its pid.
- A ticket whose holder is dead is removed by whoever sees it. A waiter that was killed or
  lost its connection blocks nobody.
- A lock whose holders are all dead is reclaimed by any waiter, and the reclaim names the
  dead pids.
- A lock directory with no pid file in it was taken by a waiter of the old scripts, and
  nobody can check it. It is waited for and never reclaimed.

The paths and the ticket names are the ones `~/dev/.derive-offload-v2.sh` used. A v2 waiter
and a waiter of this command therefore see each other's tickets while sessions move over. A
v1 waiter still races for the lock until its session stops using it.

## The round trip

1. **What travels.** The worktree as it stands: HEAD when it is clean, otherwise a
   `git stash create` snapshot of its tracked and staged changes. That writes a commit object
   and touches neither the index, the worktree nor a hook. The commit hook refuses a tree that
   is not derived yet, so the change cannot be committed before its derive. Untracked files
   would not travel and are refused.
2. **A merge in progress** travels as a real merge commit with both parents, written by
   `git commit-tree` from the staged tree once every conflict is resolved. A stash would drop
   the second parent, and the changelog, which is composed from the log, would be derived
   without the merged commits (PR #854, 2026-10-10). The derived files come back staged into
   the merge, and `git commit` concludes it through the hook.
3. **The bundle** holds only the commits `base` does not reach, and it goes over ssh's
   standard input. An input `base` already reaches is not bundled, because the host has it.
   Nothing is pushed to origin, so no CI run starts.
4. **On the host** the derive runs in a detached worktree `offload-<tag>`, in its turn. It
   builds in a **build directory of its own**: a copy-on-write clone of the shared one where
   the filesystem has clones (`cp -c` on APFS, `cp --reflink=always` on Linux), otherwise an
   empty one. The directory is removed when the derive ends. A build into the shared
   directory by a derive that never took the lock therefore cannot replace this derive's
   executable. It did on the second MacBook at 00:25Z on 2026-10-11, and stage C refused
   with "this executable is not of this tree's generation".
5. **Back here** the files the derive changed are checked out of the derived snapshot and
   staged, and the worktree is compared with that snapshot. A derive that deletes a tracked
   file is refused before anything is applied. The bundles are removed from the host, and so
   is its worktree unless `--keep` is given or the derive failed.

The machine's own derive lock (`project.one-derive-at-a-time`) is not taken: it guards this
machine, and this machine is not deriving. On the host, `scripts/derive` takes that host's
machine lock as usual.

## The board and the mesh

The offload is a shared resource, and the peer board and the mesh are how sessions share it:

- Before a long derive, say so in your `majordomus_announce` intent, with the host if you
  named one. That way a peer reading `majordomus_peers` knows why the queue on lundra is long.
- `offload probe` is the measurement. Read it rather than guessing which host is free, and
  never take the lock by hand: a hand-made empty lock directory is one the queue must wait
  for, forever.
- A session on another machine offloads to the same hosts and joins the same queues, so the
  queue is the cross-machine coordination. No mesh claim is needed for an ordinary derive.
  When work on a host must not overlap across machines, such as maintenance that empties its
  shared build directory, claim it exclusively with `majordomus_mesh_claim` first.
- Never run `scripts/derive` on a host by hand while offloads run there. It takes neither the
  queue nor the offload lock.

## Tests

<div class="overflow-x-auto" tabindex="0">

| file | proves |
|---|---|
| `apps/majordomus-cli/src/offload/queue.rs` (unit) | the decision over observations: order, dead tickets, a dead lock reclaimed, an uncheckable lock waited for, a lost ticket, an unreadable answer refused |
| `apps/majordomus-cli/src/offload/host.rs` (unit) | host choice: least loaded, ties by declaration order, unreachable, under its minimum, unknown space, a host's own minimum |
| `apps/majordomus-cli/src/offload/declaration.rs` (unit) | the declaration: defaults, unknown keys refused, addresses and paths validated |
| `apps/majordomus-cli/tests/offload.rs` | the queue against a real directory with real holder processes, and the command end to end against hosts that are directories on this machine, reached through a local stand-in for ssh |

</div>


The end-to-end tests need no network. `MAJORDOMUS_OFFLOAD_SSH` names the stand-in, which
runs the remote command under a fake host's home. A host's free space is what its stand-in
`df` reports, and a host that is down refuses the connection with ssh's exit 255. They prove
these behaviours:

- two waiters are served in arrival order, although the later one started waiting first;
- a dead waiter's ticket does not block the queue;
- a host that is down or short of space is skipped;
- the uncommitted change travels, and the result comes back staged and uncommitted, with
  nothing left on the host;
- a merge in progress keeps both parents;
- untracked files, an unresolved merge, a deleting derive and a failed derive are each
  refused;
- a concurrent build into the shared directory does not replace the derive's executable.

Each of these mutations fails a test: a lock taken regardless of position, dead tickets kept,
and `CARGO_TARGET_DIR` pointed at the shared directory.
{% endraw %}
