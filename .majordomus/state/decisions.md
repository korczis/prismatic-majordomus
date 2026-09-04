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
