+++
title = "Work moves through seats held as mesh claims, lands in combined tops, and every checkout is kept current"
description = "Work moves through seats held as mesh claims, lands in combined tops, and every checkout is kept current"
weight = 161
[extra]
kind = "rule"
slug = "project-the-cooperation-model-is-the-owners-word-1"
identity = "project.the-cooperation-model-is-the-owners-word@1"
status = "active"
source = ".ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-16 more than a hundred pull requests were open, nine or more sessions worked one
machine, every landing put every other head behind master again, the primary checkout served
plan answers from a tree 779 commits behind master, and releases waited for milestones while
master moved past them. The owner's answer, quoted with its date and an English reading in ADR 0167, was: clear
milestones, issues and waves very fast; deploy, bump and verify often, from now on always;
use the mesh; split the work and cooperate; land combined tops; always update, rebase or
bring forward whatever helps, after preserving it; document it on the site and in the UI; and
make all of it derived, generated, dynamic and enforced.

What makes that safe is not new. Decisions are still taken against the current master and
named heads (`project.integration-follows-the-current-master`), the protection is still spent
only on a judged tree (`project.an-expedited-landing-is-the-owners-word`), and paths are still
claimed before they are built (`project.work-is-claimed-before-it-is-built`). What this rule
adds is who does what, how large a landing may be, and that nobody waits to be asked before
bringing a stale tree current.

## Required behaviour

1. **A seat is an exclusive mesh claim.** A session holds a seat by holding its claim through
   `mesh.claim`, releases it when it stops, and loses it when its heartbeat stops. Nothing else
   makes a session a seat: not a name on the board, not a sentence in a handover. The seats and
   the scope each one claims:

   | seat | claims | does |
   |---|---|---|
   | landing seat | `campaign/*/landing` | lands every green head, alone, pinned to the head CI judged; builds combined tops |
   | release seat | `release/seat` | bumps, tags, records and verifies the site after landings, whenever master holds landed work past the last tag |
   | repair lane | `refs/heads/*` | merges master into a pull request that conflicts, on that pull request's own branch, one claim per branch |
   | worker | the paths of its one issue | one issue, one pull request: announces on the board, claims on the mesh, asks for a mesh review, hands the green head to the landing seat, never merges |

   The Mesh screen of the UI (`/ui/mesh`) reads these scopes from the live claims of
   `mesh.state` and draws who holds each seat now; its seat table and this table name the same
   scopes, and case 3150 and the screen's own test hold the two to each other.
2. **A combined top is the landing seat's unit of work.** Its members are pull requests whose
   heads conflict with nothing in authored paths, decided by a chain of `git merge-tree` from
   the current master. The derived files are regenerated once, after the merge commit. One
   validation run judges every member. A red member is dropped from the top and fixed on its
   own branch, never inside the top. The top lands by one merge,
   `gh pr merge <n> --admin --merge --match-head-commit <sha>`, and only when that exact head's
   `ci` concluded success and the head contains the current `origin/master`. A release's bump
   may ride inside a top. The pull request of a top names its members and their heads.
3. **Always current, preserved first.** A checkout or branch found behind master is brought
   current by whoever finds it, without asking. Before anything is moved, what it holds is
   preserved: uncommitted work as `git stash create` recorded under `refs/rescue/<name>` by
   `git update-ref`, anything else as a patch under `<repository>-rescue/`, the primary
   checkout's sibling. A checkout, or a branch no other session or forge reads, may then be reset,
   rebased or fast-forwarded. A pushed branch is brought current by merging master into it and
   pushing a fast-forward; it is never force-pushed.
4. **Release often.** The release seat cuts a release whenever master holds landed work past
   the last tag, not once per milestone. A tag is still pushed only on a commit whose `ci`
   succeeded (`scripts/ci/release-verdict`), and the site is verified after it.
5. **One record, read everywhere.** The integration, mesh and release documents point at this
   rule and ADR 0167 rather than restating the seats; the site renders both from these files;
   the UI reads the seats from the mesh. A copy of the seat table anywhere else is a defect.

## What it amends, and what still holds

ADR 0101 left batches to a person, and this is the owner taking that decision: a combined top
is the landing seat's ordinary unit, not an exception. The integration executor is unchanged:
`majordomus prs drain` still lands one ready pull request at a time and never passes `--admin`,
as `project.integration-follows-the-current-master` requires of integration code. ADR 0155's
"one landing at a time" now reads "one landing seat at a time"; a landing may carry a whole
top. Every other clause of both rules holds: decisions name the master and heads they were
taken against, no check, hook or gate is skipped, `--no-verify` is never used, a release is
never expedited, and every decision leaves its evidence.

## Enforcement points

- `test/cases/3150_the_cooperation_model_is_one_record.sh`: the seat scopes this rule names
  are the ones the Mesh screen classifies, the merge it documents is pinned to a judged head
  that contains master, and the integration, mesh and release documents point at this rule
  and ADR 0167 without copying the seat table.
- `apps/majordomus-cli/src/ui/mesh.rs`, its unit test `every_seat_is_named_by_the_rule`: the
  converse, every seat the screen draws is a scope this rule names.
- `test/cases/2999_delivery_is_expedited_only_where_the_rule_says.sh` and
  `test/cases/720_integration_follows_the_current_master.sh`: the executor still never names
  `--admin`, and a release still waits for its verdict.
- Carried by other workers on 2026-10-16 and named here as they land: a new worktree starts
  from the remote trunk rather than a stale local master (`majordomus worktree create`, branch
  `fix/a-new-worktree-starts-from-the-remote-trunk`); `majordomus start` makes sure the
  checkout's server runs before a task begins; and a checkout that has fallen behind master is
  detected and named, so clause 3 has a starter.

Clauses 2 and 3 are also acts of a session, and are decided by the record: the landing claim
on the mesh, the members and the judged head named in the top's pull request, and the rescue
refs left before a move.

## Verification

`bash test/run.sh 3150_the_cooperation_model_is_one_record`, the same for cases 2999 and 720,
and `cargo test --lib ui::mesh`.
{% endraw %}
