# Decisions

Append-only. One dated entry per decision. Newest at the bottom.

<!--
## YYYY-MM-DD — <one-line title>
Task: <task id>
Decided: <what>
Rejected: <alternatives and why>
Evidence: <file, test, or measurement>
-->

## 2026-09-04 — Context assembly reads the profile context block instead of leaving those fields unread
Task: t-20260903235710-5e89
Head: 47723904109b4ab9a6db6d3065ee640ef17fa2ab
Why: the repository rule is that every state field is both written and read; the context toggles were written by init and read by nothing
Rejected: a separate context.yaml, which would be a second source of truth for the same toggles
Evidence: test/cases/13_context.sh
Supersedes: -

## 2026-09-04 — The context builder reads the profile context block; those fields become state rather than documentation
Task: t-20260904005910-0194
Head: 359b45e2d16cfe9d29daecb6822090d1405a9b86
Why: the repository's own rule is that every state field is both written and read, and the toggles were written by init and read by nothing
Rejected: a separate context.yaml, which would be a second source of truth for the same toggles
Evidence: test/cases/23_context.sh
Supersedes: -

## 2026-09-04 — Resolution ties inside one second are broken by ledger position
Task: t-20260904005910-0194
Head: 359b45e2d16cfe9d29daecb6822090d1405a9b86
Why: created_at has second resolution, so two records written in one second would otherwise be ordered by a random filename suffix; the ledger is append-only and written in command order
Rejected: sub-second timestamps, which bash 3.2 and BSD date cannot produce portably
Evidence: test/cases/25_continuity_lifecycle.sh
Supersedes: -

## 2026-09-04 — Retrieval is a literal grep with no index or embedding
Task: t-20260904005910-0194
Head: 359b45e2d16cfe9d29daecb6822090d1405a9b86
Why: the corpus is a handful of Markdown files and one JSONL; an index would be a second source of truth that can fall out of step, and an embedding would breach the no-network and no-model rules
Rejected: a vector store; a rebuildable index
Evidence: docs/claims/semantic-retrieval.md
Supersedes: -

## 2026-09-04 — Issue and milestone status is derived and never stored
Task: t-20260904031811-67e2
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: a stored status is a second opinion that can contradict the dependency graph, and the graph is the only thing that can be right
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — Evidence lives inside the record whose completion it gates
Task: t-20260904031811-67e2
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: a separate evidence store lets the contract be read without the proof; one file keeps them inseparable
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — The GitHub adapter lives in scripts, not lib
Task: t-20260904031811-67e2
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: bin, lib, share and test contain no network client and 08_no_forbidden_constructs proves it; the model comes from lib/project.sh and only the adapter talks out
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — A milestone does not list its issues
Task: t-20260904031811-67e2
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: an issue names its milestone and the relation is read in one direction, so the two records cannot disagree about which issues belong to the outcome
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — A question blocks a completed finish anywhere in its branch, not only in the task that opened it
Task: t-20260904065911-a4c1
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: the blocker gate read only questions belonging to the active task, so a handover orphaned every blocker: the store still said unresolved and nothing was stopped. Widening the gate to the whole store fails loudly (an old question blocks visibly and is cheap to correct) where the alternatives fail silently, which is the defect itself. The store is tracked, so git already scopes it to the branch, and nothing new has to be stored to know what a question covers
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — Rejected: transferring an unresolved question to the task that succeeds a handover
Task: t-20260904065911-a4c1
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: it needs either mutation of the store, which destroys the provenance of who asked, or a lineage field, which is new state; and it is wrong whenever a handover passes work to something unrelated, blocking a task the question was never about. Its failure mode is silent, which is what we are fixing
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — Rejected: gating on scope overlap between the question's task and the finishing task
Task: t-20260904065911-a4c1
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: it is a heuristic standing in for topical relevance, and this repository refuses to guess rather than parse. It also depends on the archived task record still existing, so a pruned or pre-archive record makes the gate fail open, which is the class of defect being repaired
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — The widened gate must skip the template comment block and must let any question be resolved
Task: t-20260904065911-a4c1
Head: 41d0b364f2c21e6ea00f225f806f51c62b1e609b
Why: every fresh install ships open-questions.md with a commented example line that matches the unresolved pattern, so a naive repository-wide grep blocks the first completed finish anybody attempts; and question resolve is bound to the active task today, so a widened gate would refuse work nobody could unblock. Both are conditions on I0103, found by reading the live store rather than the code
Rejected: -
Evidence: -
Supersedes: -

## 2026-09-04 — The session envelope is derived from the ledger at close, not accumulated by other commands
Task: t-20260904153733-fc51
Head: 3c9ba2f1c9ffc2340fd9b297db3a3a4abb95d5e4
Why: the ledger is already append-only, already written only by Majordomus, already ordered by the order the commands ran, and already validated, so it holds every fact the envelope needs; deriving costs one read at close instead of a write on the hot path of every other command
Rejected: each of checkpoint, decision, question and plan appending its reference to the open session file as it runs — rejected because it creates a second mutable account of events the ledger already holds, which is the second source of truth the session record exists to avoid being, and because it makes four commands fail when a session file is malformed
Evidence: docs/SCHEMAS.md session record section; the cost is stated: the ledger becomes load-bearing for a second purpose, and equal-second events are ordered by ledger line order, the tiebreak the resolver already uses
Supersedes: -

## 2026-09-04 — The open session record is untracked; closed session records are tracked
Task: t-20260904153733-fc51
Head: 9c13909cabeb1aeb70a058f3427489307a65571a
Why: an open session carries nothing another checkout needs — unlike a task record, whose scope claim is read by other worktrees — so committing it would only make every checkout on the branch inherit an episode it did not open
Rejected: tracking it for symmetry with current.yaml, which reproduces a hazard already observed in this repository: a stale committed task record blocked every worktree until somebody closed it by hand
Evidence: .gitignore, and the foreign-record path in lib/session.sh with its case in test/cases/60_session_lifecycle.sh, which keeps the defence in place for anyone who commits one anyway
Supersedes: -

## 2026-09-04 — Every ledger line carries the session that wrote it, and the envelope selects by that stamp rather than by a time range
Task: t-20260904172432-445e
Head: 7ca51d2ce8c108364866050a7b9ca0ce5dd01d88
Why: the first real run of the time-window implementation collected another worker's tasks, checkpoints and handovers, because the ledger is one file per repository and nothing in a timestamp tells two concurrent workers apart; which episode wrote an event is a fact the machine knows at write time, so it is recorded like head and branch rather than reconstructed afterwards
Rejected: selecting every line from the session's own start event to the end of the ledger, which is what produced the misattribution; and filtering that window by branch, which would be right until a worker changed branch mid-episode and would then silently drop its own later events
Evidence: test/cases/61_session_envelope.sh — the regression section injects an event stamped by another session and one stamped by none; the mutation back to a time window was run and the case fails on it, at the assertion that a second episode must not inherit the first one's checkpoints
Supersedes: -
