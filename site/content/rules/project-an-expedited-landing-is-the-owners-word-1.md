+++
title = "An expedited landing is the owner's word, judged on the merged tree, and never reaches a tag"
description = "An expedited landing is the owner's word, judged on the merged tree, and never reaches a tag"
weight = 83
[extra]
kind = "rule"
slug = "project-an-expedited-landing-is-the-owners-word-1"
identity = "project.an-expedited-landing-is-the-owners-word@1"
status = "active"
source = ".ai/repo/rules/project/an-expedited-landing-is-the-owners-word.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-12 the published site named a release two versions behind the code for most of a
day, and the owner wrote "almost 24h with no release on gh pages + binaries, this is
tragic". Nothing was broken in the work. The required check takes about an hour on the
hosted runners, the pool judges one pull request at a time, and a record pull request that
changed one file waited two and a half hours behind runs nobody was going to read. The owner
then gave a standing order, in these words, on 2026-10-12 and again on 2026-10-13:
"deploy using --admin, bump, deploy, verify, rinse and repeat" and "use --admin mode, merge
and deploy asap, review and cancel staled jobs, be agressively efficient in delivery".

`project.integration-follows-the-current-master` says a pull request is never merged with
`--admin`, and for the integration executor that stays true: no code of this repository
passes it. What that rule did not have is the case where the person who owns the branch
protection decides to spend it. Twenty-one hours of landings under that order showed what
makes it safe and what does not, each measured rather than argued:

- **A head judged alone is not the tree that lands.** A branch whose own checks were green
  turned master red seven times in one evening, because master had moved under it. What has
  to be judged is the head with the current master merged in.
- **Two landers at once undo each other's judgement.** A merge between another lander's
  gates and its merge makes those gates a verdict on a tree that no longer exists.
- **A gate nobody ran reads as a gate that passed.** A landing went in with the structure
  job unfinished, and its two reds reached master. Saying which gates ran, and on which
  commit, is what lets the next reader know what is unknown.
- **A run nobody will read still holds a runner.** A branch's run kept going after its pull
  request merged, and the trunk's run of the merged commit waited behind it.
- **Two carriers of one red cost a disk.** Twice in one night two sessions built the same
  fix; the second build took the machine to one gibibyte free.
- **A landing between a tag and its record delays the release it follows.** The record of
  v0.21.0 waited two and a half hours behind landings that moved the derived files under
  it, and the site named v0.20.0 for all of them.
- **A release is the one thing that cannot be taken back.** A merge is corrected by the next
  merge. A tag publishes binaries that people install.

## Required behaviour

1. **The authorisation is the owner's and is recorded.** The order is quoted, with its date,
   in the decision that introduced this rule or in a later decision record. A session does
   not infer it, extend it to another repository, or take it from another session's relay: a
   relayed order is a claim about what the owner said, and is acted on only when the owner's
   own words are on record.
2. **A named lander, one at a time, under the landing claim.** The lander holds the
   exclusive mesh claim for the landing before it merges and releases it after. Nobody else
   merges to the trunk while the claim is held.
3. **The head contains the current master and is pinned.** Master is merged into the branch
   locally, the merge is committed, and only then is the branch derived. The merge is `gh pr
   merge --admin --merge --match-head-commit <head>`, so a head that moved is refused.
4. **The merged tree is judged before the merge, and the judgement is named.** At least the
   whole structure job (`scripts/ci/run-plan --job structure`), the library tests, the
   rustdoc build when the crate changed, and every case the change touches, with skips
   refused. The lander says which commit each ran on. A gate that did not run is named as
   not run. A red is not landed.
   **What a change owes beyond that is read from what it touches**, and `scripts/ci-plan
   --files <list>` names the gates it selects:
   - code of the crate (`apps/majordomus-cli/**`): the crate's tests in all three lanes of the
     rust job, not the library alone;
   - a record a crate test reads — a skill, a rule, a claim, a capability, the quality
     baseline, an intent, milestone or issue: the crate tests that open this repository
     itself — `tests/skills.rs` (it names every active skill), `tests/quality.rs` (the
     quality baseline), `tests/mcp_prose.rs`, `tests/mcp_conformance.rs`, `tests/redaction.rs`
     — and the `intent-check` gate (`majordomus intent validate`) for a plan record; a record
     change is a crate change for them although no Rust moved, and they run only in the rust
     lanes, which the structure job is not;
   - a workflow or a script under `scripts/ci/`: the whole structure job and every case that
     names the file (`grep -rl <name> test/cases`);
   - the site, the UI or the Cockpit: the site job's build and probe, and every case that
     names them.
5. **The integration executor never passes `--admin`.** `majordomus prs` merges only what
   the branch protection allows. This rule governs a lander's own act and changes nothing in
   that code.
6. **Every landing is verified where the public reads it.** After the merge the lander
   confirms the commit production serves (`scripts/pages verify`), as
   `project.land-and-publish` clause 5 requires.
7. **A run nobody will read is cancelled.** A run is stale when its pull request is merged or
   closed, when its head is no longer the head of its branch, or when another run of the
   same workflow already judges the same commit. A lander cancels its own branch's run after
   its merge. These are never cancelled: the trunk's only run of a commit, a deployment in
   flight, a release run, and another session's run on a head that is still current.
   A branch's run that has failed a job can no longer be green, and the validation workflow
   cancels it itself from the failed job's last step; the trunk's run is left to finish.
8. **A red on the trunk has one carrier.** The integrator names it. Everyone else builds
   nothing for that red until the carrier yields or its stated time passes.
9. **Between a tag and its record nothing else lands.** The release workflow proposes the
   record of what it published as a pull request made of the record and the files derived
   from it, generated against the master of that minute. A landing in that window moves
   those derived files under it and costs the record a merge and a derive, while the site
   keeps naming the previous release. The record pull request is merged first, pinned, as
   soon as its own gates are read; then the claim is open again.
10. **A release is never expedited.** A tag is pushed only on a commit whose required check
   concluded success, and no gate, hook or check is disabled, skipped or edited to get
   there. `scripts/ci/release-verdict` is that refusal, and the release workflow runs it
   before it publishes.

## Failure behaviour

`test/cases/2999_delivery_is_expedited_only_where_the_rule_says.sh` fails when the
release workflow publishes without asking `scripts/ci/release-verdict`, when that script
accepts a commit whose check is missing, pending or failed, when a landing on the trunk
could cancel the trunk's run in flight or a deployment in flight, when the integration code
names `--admin`, or when the documented merge command loses its pin to the judged head.
`test/cases/3019_a_failed_branch_run_cancels_itself.sh` fails when a gate job of the
validation workflow does not end a failed pull request's run as its last step, or could end
a run of the trunk, the schedule or a dispatch. The omission of 2026-10-13 — an active skill
that `tests/skills.rs` and case 391 do not name — is refused in the structure job by the
`skill-naming` gate (`scripts/ci/skill-naming-check`), and
`test/cases/3020_an_active_skill_is_named_where_the_crate_test_demands.sh` proves it refuses.

Clauses 1, 2, 4, 8 and 9 are acts of a session and are decided by the record: the landing
claim on the mesh, the gates named in the pull request, and the board. A landing whose pull
request names no judged commit is a finding of review.

## Verification

Case 2999 reads the workflows, the script, this rule and `docs/INTEGRATION.md` themselves
rather than a copy of them, builds nothing and reaches no network. It was proved non-vacuous
twice: with the verdict step removed from the release workflow it failed and named each
publishing step that was no longer behind the verdict, and with `cancel-in-progress: true` in
the validation workflow's concurrency block it failed and named the file.

What the verdict decides once it is asked is `test/cases/362_a_release_follows_the_ci_verdict.sh`,
against a stand-in for the forge that answers a missing, a pending, a failed and a successful
check. `test/cases/720_integration_follows_the_current_master.sh` holds the integration code
to never naming `--admin`, and `test/cases/97_pages_fast_path.sh` holds the publication queue
to never cancelling the deploy in flight.
{% endraw %}
