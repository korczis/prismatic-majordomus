---
schema: adr/v1
id: adr-0047
kind: adr
title: The session domain is typed, its identities are not interchangeable, and closing is exactly once
status: proposed
date: 2026-09-11
tags:
  - sessions
  - continuity
  - architecture
  - identity
  - rust
related:
  - file:.ai/repo/adrs/0041-the-session-lifecycle-is-the-episodes-not-the-tasks.md
  - file:.ai/repo/adrs/0014-a-closed-session-is-a-shared-object-of-the-layer-not-a-local.md
  - file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
  - file:apps/majordomus-cli/src/session/mod.rs
  - file:apps/majordomus-cli/src/capability/builtin/session_domain.rs
  - file:share/events.yaml
  - test:test/cases/240_the_session_domain_is_typed.sh
provenance:
  origin: authored
---

## Context

ADR 0041 settled the direction: one canonical session/continuity domain service in
`apps/majordomus-cli`, owning the typed lifecycle state machine, identity, events,
freshness, persistence, recovery and projection, with `.envrc`, the provider hook shims,
the CLI, MCP, HTTP and the Cockpit as adapters and projections over it. It did not build
it, and it said why: the shell path is four thousand lines and under active change by
several workers at once. A cutover on the day four agents are editing `lib/*.sh` is a merge
catastrophe, not an architecture.

So the question this decision answers is narrower and it is the one that has to be answered
first: **what is the domain, exactly, such that the cutover afterwards is mechanical?**
Three things about the stores as they exist today decide the answer, and all three are
measurements rather than opinions.

### The stores record six identities as three

Read the writers rather than the prose. `lib/common.sh` has `mj_git_repo_id`, which is the
*absolute path* of the shared git directory, and `mj_repository_id`, which is the *remote
URL* — or, with no remote, the string `local:` followed by a hash of the worktree path.
Both are called "the repository". `mj_record_front_matter` writes `worktree:` as an
absolute path; `mj_session_close` writes `worktree_id:` as the first sixteen hex digits of
that path's SHA-256. Both are called "the worktree". An open episode record carries
`session_id: s-20260910205542-e2a6` and `provider_session: <a provider's own UUID>`, and
`lib/common.sh`'s resolution order treats the second as the key the first is stored under.
Both are called "the session".

Every one of those pairs is two different things wearing one word, and the cost is not
hypothetical: a reader that compares a local record's `repository_id` with a shared
record's finds two strings that cannot be equal and concludes the records are unrelated.
The typed model's first job is to make the six things six types, so that the compiler
refuses the comparison that prose could not.

### An episode is not a thing that happens inside a task

This is ADR 0041's finding and the model must make it unrepresentable rather than merely
unwise. The representation that caused six days of silence is the one where an episode's
lifecycle transition *reads* task state. A model in which `Episode` carries an
`Option<TaskId>` and the transition function has no task parameter at all cannot express
the defect: there is nowhere to put the `if outcome != active` that caused it.

### Closing is not exactly once

`mj_session_close` was hardened after the incident of 2026-09-10 — one episode,
`s-20260909152316-024f`, with four immutable records, closed at 21:41:35 and then again at
10:39:02, 10:39:18 and 10:39:31 the next morning, because a provider's end event can fire
more than once. The hardening is a `grep -rl "^session_id: $sid$"` over the store before
publishing. That removes the *repeat* but not the *race*: two processes that grep at the
same moment both find nothing and both publish, and `mj_publish_record` is designed never
to collide, so the second one succeeds. The scan is a check, and a check without an atomic
claim is a time-of-check-to-time-of-use window with a `git status` and a ledger walk
sitting inside it — hundreds of milliseconds wide, in a repository where a provider fires
duplicate end events fifteen seconds apart and several sessions share one checkout.

## Decision

**The typed session domain lives in `apps/majordomus-cli/src/session/`, is built
additively, and nothing in this change alters the behaviour of the running lifecycle.**
The shell keeps every write it has. The domain reads the same stores, owns the semantics,
and exposes two read-only capabilities. Its write path exists, is tested, and is reachable
only from the library through a constructor that takes the opt-in explicitly; no surface
calls it and no shell command routes through it. The cutover is the next piece of work and
its order is recorded below.

**Six identities, six types, no conversions between them.** `RepositoryId`, `CheckoutId`,
`EpisodeId`, `ProviderSessionId`, `WorkerId` and `TaskId` are distinct newtypes with no
`From` between them and no common trait that would let one be passed where another is
wanted. The repository and checkout identities are *not* reinvented: they are
[`crate::repository::git_identity`] and [`crate::repository::identity`], the digests this
executable already uses to tell one checkout's server, lease and index from another's, and
the domain records each store's own spelling beside the canonical value rather than
choosing between them. A `ProviderSessionId` is documented, named and typed as an **external
correlation id**: it is how a provider's hook finds the episode it opened, and it is never
the episode's identity, because a provider that sends no session identity still has an
episode and two providers may name their sessions the same way.

**`Episode` and `Task` are separate aggregates.** An episode carries `Option<TaskId>` and
nothing else of a task. [`Episode::may_move_to`] takes the target state and the clock; it
has no task parameter, and a transition therefore cannot be gated on task state. Task
outcome may change what an artefact of an episode *says*; there is no expression in this
model by which it decides whether one is written.

**The lifecycle is a typed state machine with six transitions and one terminal state.**
`open` is `Opening → Open`; `resume` is `Detached → Open`, and a provider's start event
arriving at an episode that is still open is that same transition seen from `Open`, which
the machine allows without a move; `checkpoint` is `Open → Open`, a transition that does
not move the state and says so rather than being absent, because an event modelled as
"nothing happened" cannot be counted and counting it is how a stopped writer becomes
visible; `detach` is `Open → Detached`; `close` is `Open → Closed`; and `recover` is
`Detached → Closed`, taken when the episode's last sign of life is older than the stranded
threshold. `Closed` is terminal: [`EpisodeState::may_move_to`] answers `false` for every
move out of it, which is the assertion the store makes before it writes rather than after.

**Closing is exactly once, and the guarantee is a filesystem primitive rather than a
scan.** Before any record is composed, the closer creates `state/sessions-closing/<episode
id>.claim` with `O_EXCL`. Exactly one caller — in this process, in another process, on this
machine — creates that file; every other caller observes it and returns
[`CloseOutcome::AlreadyClosing`] or, once the record exists,
[`CloseOutcome::AlreadyClosed`] naming the record that already stands. The claim is taken
*before* the expensive part (the `git status`, the ledger walk, the record composition), so
the window the shell's scan leaves open does not exist. A duplicate close is not an error —
a provider's second end event is that provider's normal behaviour — it is a distinct,
diagnosable outcome, and [`CloseOutcome`] has a variant for the case where a *historical*
duplicate already exists so that a caller can report the repository's existing damage
rather than adding to it.

**Typed events go into the existing ledger; there is no second store.** `EventName`
validates against `share/events.yaml`, which is the vocabulary `mj_ledger_append` already
refuses an unregistered name against, and a required-field check mirrors the same file's
`requires:`. Appends are `O_APPEND` writes under an advisory `flock` on the ledger file,
which is what makes a concurrent append from another process land whole. Reading is
line-at-a-time and *lossy-by-declaration*: a line that does not parse is counted and named,
never dropped silently and never allowed to fail the read, because a ledger with one
truncated line is still the record of everything else that happened.

**Locked append-only JSONL was evaluated and is sufficient; SQLite is refused.** The
requirement is concurrency-safe append, corruption detection and recovery — not query,
not transactions across records, not indexes. `O_APPEND` gives atomicity for writes below
`PIPE_BUF` on every platform this tool runs on and `flock` covers the rest; corruption is
detected per line and recovered per line, because the format has no cross-line state to
lose. A database would add a binary file to a store whose whole value is that a person can
`tail` it, a second thing to migrate at cutover, a locking model that does not survive a
network filesystem any better than `flock` does, and a schema that would immediately become
a second account of `share/events.yaml`. The ledger stays text.

**The capability module is `session_domain`, not `session`.** The registry composes
builtin modules and declarative kinds into one namespace, and `session` is already a *kind*
— the closed episode records under `.ai/repo/sessions/`. Declaring a module by that name
made the registry refuse to build at all, with one `module 'session' is composed twice`
diagnostic per record in the store. The collision is worth recording rather than quietly
renaming around: a module id is not private to the executable, and the next person to add
one over an existing kind should meet this sentence before they meet twelve diagnostics.
The capability ids carry the module's name (`session_domain.machine`,
`session_domain.identity`); the MCP tool names and the routes are the short ones, because a
tool name is a projection and not the identity.

**Freshness is consumed, not restated.** `fresh | aging | stale | unknown | invalid`,
`Thresholds::judge` and `epoch_seconds` arrive with ADR 0041 in
`capability/builtin/continuity.rs`. This domain does not define a second vocabulary for
age. It takes ages in seconds and answers one predicate the state machine needs —
[`Episode::stranded_after`] — and the classification stays where ADR 0041 put it. The one
piece of time arithmetic this change adds is [`crate::peers::epoch_seconds`], the exact
inverse of the `rfc3339` formatter that already lives there, placed beside it so the pair
is one thing; when ADR 0041 lands, `continuity.rs`'s copy is deleted in favour of it.

## Alternatives rejected

**Cutting over now.** The shell path is under simultaneous edit by four workers in four
worktrees. Rewriting its writes today produces a merge whose conflicts are semantic rather
than textual, in the one subsystem where a silently wrong merge is invisible until somebody
resumes from a record that is not theirs. The domain is built first, beside the shell, and
the shell is moved onto it when it is still.

**A second store for episodes.** Rejected by ADR 0041 in advance and again here. The ledger
and the record directories are the substrate. A second store is a second account of events
the first one holds, which is the class of defect this whole subsystem keeps producing.

**Reusing `ExecutionId`'s shape for `EpisodeId`.** The layer's ids are
`x-20260908T010203Z-0a1b2c3d` for an execution and `s-20260910205542-e2a6` for an episode —
different stamp shapes and different suffix lengths, because the second is written by the
shell tool and has been for months. The domain parses what is written rather than what
would be tidier; a parser that refused the ids in `.ai/repo/sessions/` would be a parser
that cannot read this repository.

**Adding a third episode listing.** `episodes.*` (the episode of an MCP connection, ADR
0043) and `lifecycle.*` (session observability) are being added in the same week by two
other workers, over the same stores. A third projection of the same list, from the module
written to stop this subsystem duplicating itself, would have been the joke telling itself.
The two capabilities here are the ones neither of those has: the machine as data, and the
identities with their spellings. Repointing the existing projections at this domain is a
step of the cutover.

**Making the write path a `Command` capability.** Every projection of the registry is
read-only, and `CapabilityKind::Command` is documented as an effect on this process's
memory and nowhere else. A capability that wrote a session record would be offered over
HTTP to a caller that does not share the checkout. The write path is library API, and at
cutover it becomes a command of the command line with `LocalReason::WritesRepository`.

## Consequences

The crate gains a public module and therefore new obligations to `majordomus quality`:
documentation, runnable examples and module coverage. Every public item added here carries
a doctest.

`peers::epoch_seconds` and `continuity::epoch_seconds` will both exist for as long as ADR
0041's branch and this one are both unmerged. That is a known, textual, one-hunk conflict
and it is recorded here so that whoever merges second deletes the continuity copy rather
than renaming one of them.

**The cutover, in order.** Each step is independently landable and independently
revertible.

1. `majordomus session close` (shell) delegates to the domain's closer. This is the step
   with the measurable benefit — the race closes — and the smallest surface.
2. `majordomus session start` delegates to the domain's opener; the shell keeps the
   argument parsing and the human output.
3. The provider hook shims call the domain directly instead of `capture session`, so the
   receipt events of ADR 0041 are written by the thing that knows whether the work
   afterwards completed.
4. `checkpoint` and `handover` record against the episode rather than the task, which is
   ADR 0041's own change and is already on its branch; at that point the shell's
   `lib/checkpoint.sh` and `lib/handover.sh` become argument parsing and rendering.
5. The read side — `continuity.state`, `lifecycle.*`, `episodes.*` and the Cockpit — is
   repointed at the domain's aggregates, and the duplicated resolution in `continuity.rs`
   and `mj_resolve_latest` is deleted.
6. `lib/session.sh`'s state machine, its resolver and its freshness arithmetic are removed.
   Nothing in `lib/` holds domain state after this step.
