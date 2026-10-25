+++
title = "Work moves through seats held as mesh claims, a worker lands its own green head, and every checkout is kept current"
description = "Work moves through seats held as mesh claims, a worker lands its own green head, and every checkout is kept current"
weight = 169
[extra]
kind = "rule"
slug = "project-the-cooperation-model-is-the-owners-word-2"
identity = "project.the-cooperation-model-is-the-owners-word@2"
status = "active"
source = ".ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md"
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

### The relaxation of 2026-10-20 (version 2)

Version 1 made every landing the landing seat's: a worker handed its green head over and never
merged, builds waited for the landing seat's slot order (at most three, a load stop, a 50 GiB
floor, one queued derive), and a worker waited for a seat's word before acting. By the evening
of 2026-10-20 those waits were the bottleneck. At about 23:13 UTC the owner wrote, to session d3
and to session 0c in the same words: "hlavne prosim tebe reflektuj zmekceni pravidel, vse
updatuj, vsem dej vedet, updatni nataveni" — "above all, reflect the relaxation of the rules,
update everything, let everyone know, update the settings". Asked by d3 which relaxations he
meant, he chose all four offered: the build and disk limits, landing and merge, the sessions'
permissions, and the coordination formalities. The seats read that on the board (claim
`owner-relaxation-10-21`) as: at most five concurrent builders, a hard floor of 35 GiB free,
ids and claims still bind, and a worker lands its own green pull request through the guarded
lander's queue. That reading is what version 2 records. What did not relax: a landing is still
pinned to a head whose `ci` succeeded and which contains the current master, nothing skips a
check or a hook, and paths and identifiers are still claimed before they are built.

## Required behaviour

1. **A seat is an exclusive mesh claim.** A session holds a seat by holding its claim through
   `mesh.claim`, releases it when it stops, and loses it when its heartbeat stops — and,
   while its server beats on, a seat whose holder waits on a dialog, hangs or crashes is
   suspended within `cooperation.session_ttl_seconds` and no longer refuses the next
   session; a holder that waits on something outside itself says so with `mesh beat
   --phase` (I3990, `project.an-exclusive-claim-lives-by-its-holders-progress`). Nothing else
   makes a session a seat: not a name on the board, not a sentence in a handover. The seats and
   the scope each one claims:

   | seat | claims | does |
   |---|---|---|
   | landing seat | `campaign/*/landing` | composes and lands combined tops when heads must ship together, pinned to the head CI judged; runs the guarded lander's queue |
   | release seat | `release/seat` | bumps, tags on master's own green ci, records and verifies the site after landings, whenever master holds landed work past the last tag |
   | repair lane | `refs/heads/*` | merges master into a pull request that conflicts, on that pull request's own branch, one claim per branch |
   | worker | the paths of its one issue | one issue, one pull request: announces on the board, claims on the mesh, asks for a mesh review, lands its own green head through the guarded lander's queue |

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
6. **A worker lands its own green head.** A worker whose pull request is green enters its
   number in the guarded lander's queue (`.e7-land-queue`, beside the checkouts) and does not
   wait for a seat. The lander fast-forwards master only when the newest `ci` check run on the
   pull request's head concluded success, that head is still the pull request's head, and it
   contains `origin/master`; anything else stays queued and is named. A head that must ship
   with others still goes to the landing seat as a member of a combined top (clause 2).
7. **Builds are bounded by the machine, not by a slot order.** At most five builders run at
   once on one machine, and none starts while less than 35 GiB is free. Within those limits a
   session builds when it needs to, and says so on the board; it does not ask for a slot.
   Identifiers, paths and seats are still claimed on the mesh before they are used, and after
   every completed step a session tells the peers it affects.
8. **Cleanup that loses nothing is done without asking, and announced.** A worktree whose
   branch is merged, whose tree is clean and in which no process works is removed with
   `git worktree remove`, never `--force`; Docker's build cache, anonymous volumes and dangling
   images are pruned. Each removal is announced on the mesh. Anything that holds unlanded or
   uncommitted work is preserved first (clause 3) and is not cleanup.

Two things this rule does not grant. The Claude Code permission rules of the machine
(`autoMode.allow` and the settings files) are the owner's to write: a session hands the owner
the text to paste and never edits them, so no document claims more autonomy than the settings
give. And the local pre-commit and pre-push gates are judged by their own issue in the CI
critical path, not by this rule.

## What it amends, and what still holds

Version 2 lifts two words of version 1's worker row, "never merges": a worker's own green head
is landed through the guarded lander's queue, under exactly the pin the landing seat lands
under, so nothing about what may reach master is looser. It replaces the landing seat's slot
order with the two machine limits of clause 7. Everything below holds as in version 1.

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
- Carried by other workers on 2026-10-16, each its own pull request, and named here so that
  clause 3 has a starter: a new worktree starts from the remote trunk rather than a stale local
  master (`majordomus worktree create`, pull request #1291, case 3131); `majordomus start`
  makes sure the checkout's server runs before its task claims its scope on the mesh (pull
  request #1288, case 3151); and a checkout reports how far it stands behind the remote trunk
  wherever it is read (rule `project.a-checkout-serves-the-current-trunk`, branch
  `feature/a-checkout-serves-the-current-trunk`, case 3176). When they land, their cases join
  this rule's tests.

- `test/cases/3824_the_bootstrap_says_what_the_cooperation_rule_says.sh`: the bootstrap
  paragraph every provider's instruction file carries says what clauses 6 and 7 say (a worker
  lands its own green head through the guarded queue; five builders, 35 GiB) and no longer says
  "never merges", each provider template carries the same paragraph, and it names this rule.

Clauses 2, 3 and 6 are also acts of a session, and are decided by the record: the landing claim
on the mesh, the members and the judged head named in the top's pull request, the queued pull
request and the head the lander pinned, and the rescue refs left before a move. Clause 7's two
limits are, today, the seats' operating numbers read from this record; the build guard in
`lib/rust_bin.sh` still refuses only below its own 5 GiB default, and holding a build to 35 GiB
and five builders in code is owed (I3825).

## Verification

`bash test/run.sh 3150_the_cooperation_model_is_one_record`, the same for cases 2999, 720 and
3824, and `cargo test --lib ui::mesh`.
{% endraw %}
