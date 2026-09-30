# Pull-request integration

Majordomus does not merge a list of pull requests. It integrates the next provably safe
change into the current master, verifies that it landed, discards what it assumed, and
decides again from what is there now. The decision is recorded in ADR 0101, the rule is
`project.integration-follows-the-current-master`, and the code is `crate::integration`.

## The pipeline

```text
forge (gh) ──► observation (recorded, with its moment)
                   │
git merge-tree ──► relation to master (with this clone's merge drivers; cached by SHA pair)
                   │
classify ─────────► one assessment per pull request: disposition, reasons, evidence, risk
                   │
rank ─────────────► the queue: deterministic order, the next merge, the next refresh
                   │
CLI · HTTP · MCP · Cockpit render it, offline
                   │
drain ────────────► lease ► observe ► decide ► observe again ► act ► verify ► (repeat)
```

The forge's `mergeable` flag is never read. This repository resolves derived files with a
per-clone merge driver (`merge=derived`) that the forge cannot run, so the forge calls nearly
every pull request conflicting. The relation to master is decided by git: `git merge-tree
--write-tree` with the drivers, and `git check-attr merge` for which paths are derived,
according to master's own `.gitattributes`.

## Dispositions

Every open pull request has exactly one. They are decided in the order below, so an earlier
answer wins. `ready` is reached only after every other question is answered in its favour.

| Disposition | Lane | When | Next |
|---|---|---|---|
| `other_base` | held | targets a branch other than the base, and no open pull request's head | — |
| `waiting_for_dependency` | waiting | stacked on another open pull request, or declares `Depends on #N`/`Stacked on #N`/`Requires #N`/`After #N` on an open one | land that one first |
| `draft` | held | a draft | mark it ready |
| `blocked` | held | carries a blocking label (`do-not-merge`, `blocked`, `hold`, `on-hold`, `wip`, `manual-merge`) | remove it |
| `superseded` | cleanup | its head is an ancestor of master, or merging it changes no file | `prs cleanup --apply` closes it |
| `possibly_redundant` | cleanup | merging it changes only derived artifacts | a person decides |
| `unknown` | held | its head is not fetched, git failed, or the branch protection could not be read | `prs refresh` |
| `conflicting` | repair | the merge conflicts on an authored path | the author resolves it |
| `waiting_for_review` | waiting | a required review is missing or changes were requested | a reviewer |
| `needs_repair` | repair | a required check failed on its head, or it is behind master from a fork | the author |
| `needs_refresh` | waiting | merges cleanly but does not contain master | `prs drain --refresh` |
| `waiting_for_checks` | waiting | contains master; a required check is pending or missing on this head | wait |
| `ready` | ready | contains master, and every required check passed on this head | `prs drain` |

A required check that is pending, missing, skipped or unreadable is not passed. The required
checks are read from the base's branch protection, never listed here. A green check that is
not required proves nothing.

## The rank

The queue is ordered by lane, disposition, risk (low, medium, high, from the paths touched),
how many other ready or refreshable pull requests share an authored path (fewer first,
because landing it invalidates less), age (older first, so new easy work cannot starve old
work) and number. Every key is a value of the assessment, so the order is total and does
not depend on the order the forge listed them. `tests/integration_queue.rs` proves this as a
property.

## One merge at a time

`prs drain` loops over one step and holds nothing between steps:

1. observe the forge, fetch every open head, and build the queue;
2. take the first `ready` pull request;
3. observe again and rebuild the queue;
4. act only if the second decision names the same master and head and still says `ready`,
   and otherwise record `stale_decision` and start again;
5. merge with the repository's merge method and `--match-head-commit <head>`, never `--admin`;
6. verify that the forge shows it merged and that the fetched master contains its head;
7. record the merge with the master before and after.

A merge moves every other pull request behind master, so the next step always starts from a
new observation.

When nothing is ready, `prs drain --refresh` brings master into the first `needs_refresh`
pull request. It uses a scratch worktree under the common git directory, runs `git merge
--no-commit` with the derived driver, runs `scripts/derive`, commits with the hooks running,
and pushes a plain fast-forward to the pull request's branch. That is a merge commit and never
a rewrite, as `project.land-and-publish` prescribes. The pipeline is one deep: while a
refreshed pull request waits for its checks, no other is refreshed, because merging the first
would put the second behind again. Throughput is therefore one pull request per run of the
required check, which is the true cost of this repository's mechanics.

## Cleanup

Closing a pull request requires more evidence than merging one. `prs cleanup` lists the
`superseded` ones and closes them only with `--apply`, with a comment that names the master
and head that proved it. `possibly_redundant` is listed and left for a person. Age, shared
paths and similar titles are not evidence of anything. Branches are not deleted. The
repository's own setting decides that.

## Safety

- One executor per base branch: `drain` and `cleanup --apply` hold an exclusive lease at
  `<git-common-dir>/majordomus/locks/integration-<base>.lock`, with the holder recorded. A
  lease untouched for 30 minutes is reclaimed. Observers never take it.
- Every act is appended to `.ai/local/state/integration/events.jsonl`: `selected`,
  `stale_decision`, `merge_attempted`, `merge_succeeded`, `merge_failed`,
  `verification_failed`, `refresh_selected`, `refreshed`, `refresh_failed`,
  `closed_superseded`, `idle`.
- A dry run observes and decides, and changes and records nothing.
- A refused merge and a stale decision are specific to the candidate: the next step
  re-plans. A verification failure stops the drain.

## Commands

| Command | Network | What |
|---|---|---|
| `majordomus prs` / `prs status` | no | the ranked queue; exit 10 when the observation is stale or absent |
| `majordomus prs plan` | no | the next merge, the next refresh, and the other lanes |
| `majordomus prs explain <n>` | no | one pull request's evidence and rank |
| `majordomus prs events` | no | the audit trail |
| `majordomus prs refresh` | yes | observe the forge and fetch every open head |
| `majordomus prs drain [--max N] [--dry-run] [--refresh]` | yes | integrate, one merge at a time |
| `majordomus prs cleanup [--apply]` | yes | close what is provably on master |

The same queue is `GET /api/v1/pull-requests` (MCP `majordomus_pull_requests`). One pull
request is `GET /api/v1/pull-requests/explain?number=` (`majordomus_pull_request_explain`),
and the trail is `GET /api/v1/pull-requests/events` (`majordomus_integration_events`). All
three are declared once in `capability/builtin/integration.rs`.

## Rollout

1. **Dry run.** `prs refresh && prs status && prs drain --dry-run` against the real
   repository. The classification was checked on 2026-09-30 against 70 open pull requests.
2. **One merge.** `prs drain --max 1` once a pull request is `ready`.
3. **Bounded.** `prs drain --refresh --max 3`.
4. **Continuous.** Only after the audit trail shows several verified cycles.
