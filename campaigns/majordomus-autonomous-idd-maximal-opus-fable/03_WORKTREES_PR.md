# PHASE 03 — WORKTREES, GITHUB PR DRAIN AND INTEGRATION

Audit existing worktree management, GH integrations, branch collision checks, CI, PR checks/reviews, merge policy, stale branch handling and local branches. Distinguish open PRs from issues, and count all WIP by upstream/GitHub state. Build on existing canonical capabilities. Do not manufacture a PR orchestration subsystem if it already exists; only close its gaps.

Use one accountable canonical work identity per issue/attempt and discover branch/worktree naming from repo. Preserve unmerged work. Reconcile worktrees against actual Git refs and PRs before cleanup; never delete dirty, unpushed or unmerged trees without explicit approval. Integrate branch conflict detection and exclusive mesh claims before task dispatch. Cap active modifications to avoid dozens of incompatible PRs. Existing PRs matching an intent criterion must be discovered before creating new work.

Develop a bounded drain loop: discover -> classify ready/blocked/draft/conflicting/stale -> prioritize -> rebase/update if safe -> run targeted tests/CI -> review/revise -> merge using repository branch rules -> verify resulting default-branch commit -> cleanup safe source/worktree. Do not bypass required checks, protected branches, reviewers, required signed commits or merge queues. Detect conflicts and mark reconciliation/blocker; automated changes can fix and rerun gates but never overwrite unknown work. External effects must have receipts and idempotent reconciliation.

Build evidence mapping between intent criterion, issue, worktree, commits, PR, review, checks, merge SHA and release eligibility. PR state alone is NOT issue completion. Apply safety policies for dependencies and release grouping, allowing multiple small PRs per intent if tests prove it is safe. Monitor stale PR age and WIP count. Auto-versioning should operate on accepted release-unit boundaries, not necessarily every PR unless canonical policy explicitly requires per-PR bump and avoids version races.

UI projections: PR queue ordered by actionable priority, blockers and owner; per-PR detail with linked intent/milestone/issue, diff URL, checks, reviewer and mergeability, retry/rebase/merge controls gated by policy, links to worktree and mesh worker, transparent operation status and audit trail. Be mobile accessible.

Tests: two concurrent PRs editing same source; pre-existing unrelated PR; stale CI; force-push race; already merged PR; duplicate webhook; unmerged dirty tree; branch protection rejection; rebase conflict; safely persisted merge receipt after coordinator crash. Include integration tests against local fake GH provider and optional authenticated live dry-run against GitHub with isolated fixtures.

Acceptance: a green scoped work item reliably reaches the default branch with checks and review accounted for; backlog declines; restart never remerges or accidentally deletes work.
