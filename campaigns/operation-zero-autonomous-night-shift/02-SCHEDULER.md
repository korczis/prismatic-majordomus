# 02 — Dependency-aware drain scheduler

Scheduling goals in order: preserve correctness; satisfy required reviews and gates; maximize *verified merged PR per unit time*; unblock stacked descendants; minimize expensive shared rebuilds. Compute a deterministic ready set from canonical graph, classify blockers, prioritize approved+green, low-conflict, short CI, high fan-out. Avoid starving difficult PRs; age candidates and periodically rescore.

Separate actors: READERS (discovery, reviews, conflict prediction), REPAIR WORKERS (isolated worktrees), INTEGRATION WRITER (serialized merge + version allocation), RELEASE/DEPLOY VERIFIER (may operate in parallel for already committed independent transactions only if canonical deployment policies allow it). All workers coordinate through the existing mesh/lease/task model, not an ad hoc shell-background swarm.

Bound concurrency using observed CPU, RAM, disk, GitHub rate limits and CI capacity. Only one owner of each worktree/branch and lease; never delete active worktree or overwrite uncommitted work. Group independent candidate validation opportunistically; invalidate stale evidence if head, base or relevant policy changed. On no-progress, diagnose queues, blocked edges, failed providers, stale leases, repeated CI fails, and rate limits; make targeted repair then re-run. Zero-ready with open PRs means a blocker report, not success.
