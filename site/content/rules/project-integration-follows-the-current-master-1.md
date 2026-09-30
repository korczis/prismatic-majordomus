+++
title = "Pull requests are integrated one at a time, each decided against the current master, never around the branch protection"
description = "Pull requests are integrated one at a time, each decided against the current master, never around the branch protection"
weight = 94
[extra]
kind = "rule"
slug = "project-integration-follows-the-current-master-1"
identity = "project.integration-follows-the-current-master@1"
status = "active"
source = ".ai/repo/rules/project/integration-follows-the-current-master.v1.md"
+++
{% raw %}
## Rationale

Seventy pull requests were open on 2026-09-30, and the repository had three competing answers
to "what can land": the forge's `mergeable` flag, which cannot run the derived-file driver and
is wrong in both directions; `scripts/land`, which merged every clean pull request onto one
branch from a list its own first merge made stale; and hand-built batches whose regressions
nobody could attribute. ADR 0101 replaces all three with one typed subsystem. This rule is
what keeps it from drifting back.

## Required behaviour

- A pull request is `ready` only when it targets the base, is not a draft, carries no blocking
  label, contains the current master, has its declared dependencies landed, satisfies the
  review policy, and has every required check passed on its current head. Pending, missing,
  skipped and unreadable are not passed.
- The executor observes the forge before deciding and again before acting, and merges only
  when both decisions name the same master and head. It holds no plan across a merge.
- No integration code passes `--admin`, force-pushes, or rewrites a branch. Bringing master
  into a branch is a merge commit pushed as a plain fast-forward.
- Closure is limited to pull requests whose head is an ancestor of master or whose merge
  changes no file, and only with an explicit `--apply`. A pull request that differs only in
  derived output is left for a person.
- Mutations hold the base branch's integration lease. Observers do not.
- Every selection, stale decision, merge, refusal, refresh and closure is appended to the
  audit trail.

## Failure behaviour

`test/cases/720_integration_follows_the_current_master.sh` fails when the integration code
names `--admin` or a force push, when a read-only `prs` command reaches the network or
writes the audit trail, when the relation to master is taken from the forge's `mergeable`,
or when the executor loses its re-observation before acting. The crate's
`tests/integration_queue.rs` holds the dispositions, the stale-decision refusal, the
re-plan after every merge and the cleanup threshold.

## Verification

`bash test/run.sh 720_integration_follows_the_current_master` and
`cargo test --test integration_queue`.
{% endraw %}
