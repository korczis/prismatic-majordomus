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
