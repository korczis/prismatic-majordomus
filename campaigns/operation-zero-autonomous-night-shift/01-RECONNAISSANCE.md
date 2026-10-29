# 01 — Reconnaissance and canonical backlog DAG

Inspect authoritative GitHub state with complete pagination; separate PRs from issues (GitHub issue endpoints include PRs). Discover all milestones including open, overdue and completed records; discover intent/criteria mapping from local canonical model. Record base/head SHA, mergeable/draft status, checks at current revision, approvals, requested changes, labels, CI state, linked issues, milestone, worktree/session/peer owner, last activity, merge dependency and release impact.

Construct typed normalized graph: nodes {PR, Issue, Milestone, IntentCriterion, Commit, Worktree, ReleaseObligation}; edges {depends_on, implements, blocks, includes, satisfies, supersedes, duplicate_of, closes, produces}. Derive from actual GitHub refs, PR bodies, commit ancestry, canonical project state and reliable repo conventions. Mark uncertain inference explicitly and do not auto-close on uncertainty.

Classify PRs: READY, NEEDS_SYNC, NEEDS_FIX, STACKED_BLOCKED, ALREADY_INTEGRATED, DUPLICATE_PROVEN, SUPERSEDED_PROVEN, EXTERNAL_BLOCKED. Verify claims with commits/diffs or repository acceptance records. First snapshot includes counts, provider errors, pagination completeness and base revision. Reconcile refreshes after each integration; a failed API call is UNKNOWN, never an empty backlog.

Find historical merged work and stale open records, verify semantically rather than merely checking patch-id when diffs materially changed. Prefer existing Majordomus PR/issue/milestone projection interfaces and canonical models. No second inventory manually maintained in markdown or UI.
