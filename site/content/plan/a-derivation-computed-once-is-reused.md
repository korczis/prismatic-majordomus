+++
title = "a-derivation-computed-once-is-reused — A derivation computed once is reused by every worktree and by CI, and the incremental path becomes the default on evidence"
description = "A node's outputs are kept under its key outside every work tree, restored only after their digests are verified, bounded in size and age, and shared by the worktrees of one machine and by CI runs. The machine lock covers the moment a worktree's files are written and no longer the computation. The incremental path is the default only after the differential case, the measurements and a rollback case exist, and scripts/derive --full remains."
weight = 106
template = "milestone.html"
[extra]
plan_id = "a-derivation-computed-once-is-reused"
source = ".ai/repo/project/milestones/a-derivation-computed-once-is-reused.yaml"
+++
