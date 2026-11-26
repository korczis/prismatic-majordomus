+++
title = "Take finished work to a verified master"
description = "Carry a finished change from an open pull request to master and beyond through the one integration engine and the one landing seat, answering at each stage why it is not there yet and stopping only at a landing verified on master, a release, or a named blocker with its next act."
weight = 5
[extra]
id = "integrator"
status = "active"
version = 7
source = ".ai/repo/skills/integrator/SKILL.md"
+++
{% raw %}

## Purpose

Implementation is not delivery. A pull request opened is a request; a green check is a
verdict on one head; only a commit on master whose required check passed is integrated, and
only a tag the release workflow published, and a site that serves it, is delivered. This
skill walks a change through those stages with the engine this repository already has — it
adds no queue, no lander and no second state.

The engine is `majordomus prs` (ADR 0101, `docs/INTEGRATION.md`): it observes the forge,
relates every head to master with git, classifies every pull request into one disposition
with its evidence, ranks them, and lands the next provably safe one by a non-forced
fast-forward pinned to the head CI judged. Every surface — the CLI, `integration.*` over
HTTP and MCP, the Cockpit, the briefing — renders that one assessment.

## When to use

- Implementation is done and pushed, and the question is "when is it on master?".
- `/proceed` finds finished work that has not landed, or landed work that is not released.
- You hold the landing seat and the queue has ready heads.
- A train failed, or a pull request has waited longer than its peers.

Not this skill: bringing a branch current (`rebaser`), deciding what to build next
(`majordomus plan next`), or recording lessons (`evolve`, which this skill feeds).

## Procedure

### 1. Where is it, and why

```
majordomus prs refresh
majordomus prs status             # every open pull request: disposition, lane, rank, waiting
majordomus prs explain <n>        # one of them: the evidence for each answer
majordomus prs plan               # what the executor would do next, changing nothing
```

Read the disposition table in `docs/INTEGRATION.md`. Every non-ready disposition names its
next act and whose it is. A pull request is never "probably fine": a required check that is
pending, missing, skipped or unreadable is not passed.

### 2. Who may move master

Exactly one session holds `seat/landing`, an exclusive mesh claim
(`project.the-cooperation-model-is-the-owners-word`, ADR 0167); `majordomus mesh peers --full`
shows it, and the integration lease (`prs brief`) shows which executor holds the base.
Branch protection requires the `ci` check; it does not require a branch to be up to date,
so the fast-forward the drain uses is what keeps master equal to a judged tree.

- **Not the seat holder:** make the head ready and hand it over. It contains master
  (`rebaser`), its required check passed on that head, it is not labelled `hold` unless it
  is a train member, and its issue and evidence are recorded. Announce the head on the board
  with `majordomus_announce` (or `majordomus mesh session`), naming the pull request and the
  sha. Do not merge it yourself, and do not re-run its checks to hurry it.
- **The seat holder:** `majordomus prs drain --dry-run` decides, then `majordomus prs drain`
  lands — one merge at a time, observe → decide → observe again → act → verify. A stale
  decision is recorded and the next step starts from the new master; never force it.

### 3. Choose the lane by what it costs

<div class="overflow-x-auto" tabindex="0">

| Situation | Lane |
|---|---|
| one independent pull request, ready | **drain**: it lands on its own required check |
| several related or derived-data-heavy members | **train**: one top combines them (`docs/INTEGRATION.md`, Trains); members carry `hold` and their own runs give their slots back (I4338) |
| a member of a red top | **split**: the top judges its members; drop the failing member to its own lane and land the rest, rather than holding everything behind one red |
| conflict on an authored path | **repair**: the owner's (`rebaser`, row `conflicting`) |
| waiting on a lower link, a review, a label | **wait**, with the blocker named |

</div>


Cost here is CI time: one required-check run took 226 minutes at the median in the week to
2026-11-01, and refresh throughput is one pull request per run. A strategy that adds a run
(a needless refresh, a re-run, a label that starts a run) must buy more than it costs.

#### Composing and judging a train top

Whoever composes a top (the landing seat, or a worker the seat gives a top to) calls it ready
when its own pre-push judgement passed, not when it is committed:

- **Build before the first merge, derive before each merge commit.** A fresh linked worktree
  holds no executable, and every hook of a merge commit asks for one of the tree's generation
  (ADR 0224): on 2026-11-05 `git merge --no-ff` of #2931 into train 27 auto-merged every file and
  the commit hook refused, leaving `MERGE_HEAD` staged. Build first (`cargo build --locked
  --manifest-path apps/majordomus-cli/Cargo.toml` in the worktree), then for each member
  `git merge --no-ff --no-commit <member>`, `scripts/derive` while the merge is staged,
  `git add -A`, `git commit`: the pre-commit pages gate refuses a merge commit whose derived
  data is stale (candidate s-20261028115702-2cb6-c77d6b5c9317, met again the same day). Then,
  once after the last member's merge commit, derive again and commit what it changes on its
  own. `site/data/generated/delivery.json` observes HEAD by design (I3836), and a staged merge
  has not moved it, so the derive before the commit observed the pre-merge head and missed
  what the merges brought in (curated note s-20261106212411-be2e-16bd427c8946); the site check
  then refuses a homepage that counts more merged issues than the committed projection. This
  holds for `majordomus offload derive .` as for `scripts/derive`, and on 2026-11-10 it cost
  two pull requests (#2994, #2998) a full validate each. Rebuild between members only when a
  member changes the crate.
- **A build host is not a judging host.** The host that builds, derives and commits a top may
  not carry CI's tools: on 2026-11-05 lundra refused train 25b's top at pre-push with
  `190_graph_drawing` ("node is not available"), `3196_stale_crate_artifacts_are_pruned_under_cargos_lock`
  and `3590_the_site_is_derived_in_every_language` red, 45 gates and 193 cases in 314 s against
  the 180 s bound, while the same head passed run-plan on a host with node and zola 0.23
  (166 s) and was pushed. Move the commits with `git bundle`, judge and push from the host
  whose tools match CI's, and say which host judged in the ready report.
- **A seat's ledgers live where a session's death cannot take them.** The flake ledger and the
  strategy protocol of the seat that vanished at 11:40Z on 2026-11-05 were board claims
  (`majordomus_announce`), which belong to an MCP connection; its mesh claims and handovers
  survived and its board did not, and the next seat could not recover train 27's READY
  contract. Keep a ledger in a mesh claim's intent (`majordomus mesh claim`), a handover
  (`majordomus handover`) or a tracked record; the board carries what is true for the
  connection, never the only copy of anything.
- **Master stays the first parent.** A top committed with `git commit` over a hand-written
  `MERGE_HEAD` loses master as a parent whenever a member already contains it, because git
  reduces redundant parents. In a linked worktree `.git` is a file, so write the mode where
  git reads it: `printf no-ff > "$(git rev-parse --git-path MERGE_MODE)"` (the file is
  compared as exactly `no-ff`; `echo`'s newline does not count). `git rev-parse HEAD^1` must
  then be master.
- **Judge before the slot.** `git push --dry-run -u origin <train-branch>` runs the pre-push
  hook and moves nothing; a push carrying an `integration/train-*` ref refuses run-plan's
  exit 1 (`.githooks/pre-push`). Report the head and run-plan's own last line, never "ok".
  `run-plan --before-push` runs only the structure and structure-b gates; the rust job's
  gates, `intent-check` among them, are first met in CI. Until
  `project.a-push-runs-what-ci-would-find-first` reaches them, a ready report also carries
  `bin/majordomus-cli intent validate` with no failure.
- **Judge where CI's tools are.** A host whose pinned tools differ from CI's fails cases on
  plain master too: zola 0.22 against the 0.23 that `.github/actions/setup-site` installs
  failed seven cases on master and on the top alike. Before a member is blamed or dropped,
  run the failing cases on the base on the same host (`bash test/run.sh <cases>` in a
  detached worktree of master); identical failures are the host's, and the top is judged
  on a host whose pinned tools match (the macOS differences of `docs/INTEGRATION.md` aside).
- **Run what the budget leaves out when a record is renamed.** `run-plan --before-push`
  runs a structure gate only when CI recorded it at 15 s or less and the sum stays within
  120 s (`.ai/repo/ci/structure-durations.tsv`), so core-check and knowledge-nodes are
  first met in CI, and adr-collision is too once cheaper gates fill the sum; the commit
  hook's doctor does not resolve a decision's `related:` paths. A top that carries a rule's
  or a decision's version raise, or any record file renamed, runs
  `bin/majordomus adr check`, `bin/majordomus knowledge nodes --scope shared` and
  `scripts/ci/core-check` itself, and greps the tree for the old path
  (`git grep -n -F <old path>`), before it is called ready. Train 19c (#2918) passed every
  local judgement and failed structure in run 41896185903: ADR 0167 still related the
  `.v1.md` file of the rule its member raised to v2 (5512734636d).
- **Judge a long-lived member's own cases on the merged tree.** A case a member added
  passes against the master it was written on and can fail on the current one, because
  master gained defaults since. Train P (#2855) met two in #2522: case 3441 met the mesh
  declaration `majordomus init` has shipped enabled since 2026-10-22, and case 99 met a
  milestone id master added beside the member's, which `lib/project.sh` ordered otherwise
  than the executable (37a8ef2f4c9). Every validate run of the member had been cancelled,
  so the top ran them first. Run the cases the member adds or changes
  (`git diff --name-only origin/master...<member> -- test/cases`) with
  `bash test/run.sh <cases>` on the merged top, not only on the member's head.

### 4. Do not starve the queue

- A pull request passed over three times is `starving` (`prs status`): put it in the next
  train or give it the seat's next refresh, do not refresh it again on its own.
- A run that yielded (`ci:yielded`) is re-requested by `scripts/ci/release-path resume`;
  do not re-run it by hand while the release path holds — the re-run yields again.
- A label other than `ci:full` or `hold` starts no second verdict (I4347,
  `scripts/ci/carried-verdict`); `ci:full` deliberately does.
- A github-projection failure that says a record is `behind` and "this head did not change
  them" is not the pull request's: either the trunk moved past its base, master's projection
  is still running, or master's projection run failed and did not apply what a landing
  carried (on 2026-11-02 at 23:02Z its checkout timed out). Read master's projection run
  first. When it failed, prefer re-running its failed job over the manual apply the rule
  keeps for a workflow that cannot (`project.the-projection-is-applied-at-landing`), so the
  records keep one writer; once it is green, the seat re-runs the pull request's failed job.
- Stack the second validating top on the first top's head, not on master. Two tops judged
  side by side on master are one slot: when train P (#2855) landed at 02:53:53Z on
  2026-11-05, train 19c (#2918) went DIRTY on the forge at once, because derived files that
  resolve to ours locally conflict there, and that cost a master merge and another round of
  about 2.5 h. Stack it when its authored files merge cleanly onto the first head, say
  "stacked on #<n>" in its body, and expect its github-projection job red until the base
  lands; otherwise compose it on master once the first has landed.
- The runner pool is shared across the account's repositories. At 04:02Z on 2026-11-05, 83
  queued jobs of eight feature-branch runs of another repository kept a train top's validate
  (run 41961637000) queued for over 35 minutes. Before pushing a top, count the queued jobs
  of every repository of the account (`gh run list -R <owner>/<repo> --status queued`, then
  `gh run view <id> --json jobs`); a deep queue elsewhere means the top waits behind it, and
  pushing a second head only adds to it. Issue I4360 plans the monitor moment for a run
  queued past its bound; the cause it names is "no runner online", which a pool busy with
  another repository is not.

### 5. Verify the landing, then the release

Landed means all of these, each read, not assumed:

```
majordomus prs events             # the landing recorded, master before and after
git fetch origin && git merge-base --is-ancestor <head> origin/master   # exit 0
gh run list --workflow validate --branch master --limit 1               # master's own verdict on that commit
```

Then delivery: `majordomus run release.currency` says whether landed work waits past the
last tag (the release seat's act), and `majordomus served` / `majordomus deploy` say what
the site serves. A merge the forge accepted is not a green master, and a green master is not
a release.

A release is not reported released until the live homepage shows it in a real browser:

```bash
scripts/release-probe --expect v<version> --wait 600 --out target/release-probe
```

renders https://majordomus.dev/ in Chromium with the cache bypassed and holds it to the tag:
GitHub's newest release is that tag and the page names its version; the trust strip, the
navbar badge, `/releases/latest.json` and the newest pre-release ahead of stable agree; the
footer names `/build.json`'s commit. `release.yml` runs the same in its `live` job after
`publish`, and the pages deploy runs it after `the release it offers`; both keep `home.png`
and `result.json` as the run's artifact. The release seat's report of a release carries the
probe's lines and the screenshot (I4530); a `FAIL` names both sides, and the next act is on
the source — the record that has not landed, the pages that read the tree's records only —
never on the picture.

### 6. Close the loop

Record the issue's evidence against the commit that landed it, and hand what the attempt
taught to `evolve`: a repeated stale decision, a train that failed for one member, a run
that yielded twice. Report the attempt's own numbers from `prs status` and `prs events`
(wait, passes over, retries), never an estimate.

## Output

<div class="overflow-x-auto" tabindex="0">

| pull request | stage reached | evidence (run, commit) | blocker, owner, next act |
|---|---|---|---|

</div>


with the stages open → ready → landed → released → deployed, each marked with the states of
`report-verification-state`. A pull request handed to the seat but not landed is PARTIAL;
nothing is done before master's own run on the landed commit has passed.
{% endraw %}
