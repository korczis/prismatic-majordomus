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
