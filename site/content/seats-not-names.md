+++
title = "Seats, not names"
description = "Nine sessions on one machine, more than a hundred open pull requests, and a primary checkout 779 commits behind its own master. Why the answer was to make every role a claim on the mesh rather than a name, to land many pull requests in one judged run, and to bring stale work current without asking."
template = "page.html"
[extra]
lang = "en"
related = ["everything-followed/", "the-bet-on-a-layer/", "a-pasted-brief-becomes-a-plan/", "a-plan-is-attacked-before-it-is-built/", "claimed-before-it-is-built/"]
[extra.translations]
cs = "kresla-ne-jmena/"
+++
{% raw %}

On the morning of 16 October 2026, 107 pull requests were open on this repository. Nine or more AI sessions were working on it at once, on one machine, and the way they landed their work had stopped keeping up with them.

The record of that morning is [ADR 0167](@/adrs/adr-0167.md). It lists three failures, each one measured rather than argued.

<div class="overflow-x-auto" tabindex="0">

| what happened | what it cost |
|---|---|
| each landing put every other open head behind master again | a merge of master, a derive and a run of about an hour per pull request, so the queue grew faster than one lander could drain it |
| the primary checkout stood 779 commits behind master | its server answered questions about the plan from that old tree, and a session cancelled a sweep of fourteen issues after finding thirteen of them already done on master |
| releases waited for milestones | the published site and binaries named a version that master had long passed |

</div>


The owner answered the sessions that morning in a few lines of Czech. Split the work and cooperate. Land many pull requests together. Bring everything up to date, with whatever helps. Deploy and release often, always. Document it, and derive and enforce all of it. The rest of this article is what those lines became.

## A seat is a claim

The first decision was about who does what. Integration needs a few roles that only one session may hold at a time: someone lands, someone releases. The obvious way to record that is a name in a document: "session 9c releases". That name is out of date the moment the session reconnects under another identity, crashes, or hands the job over, and nothing would notice.

So a role is a **seat**, and a seat is held by holding an exclusive claim on the mesh, and in no other way. A claim is already a signed event that every machine replicates, that lives only while its session's heartbeat does, and that another session cannot take while it holds. The rule [project.the-cooperation-model-is-the-owners-word](@/rules/project-the-cooperation-model-is-the-owners-word-1.md) lists four seats:

<div class="overflow-x-auto" tabindex="0">

| seat | claims | does |
|---|---|---|
| landing seat | `campaign/*/landing` | lands every green head, pinned to the head CI judged, and builds combined tops |
| release seat | `release/seat` | bumps, tags, records and verifies the site whenever master holds landed work past the last tag |
| repair lane | `refs/heads/*` | merges master into a pull request that conflicts, on that pull request's own branch |
| worker | the paths of its one issue | one issue, one pull request; hands its green head to the landing seat and never merges |

</div>


A seat moves when its claim moves, and a crashed holder's seat expires with its heartbeat. No session has to remember who holds it. The Mesh screen of the UI, `/ui/mesh`, reads the live claims and draws who holds each seat now. Case 3150 and the screen's own test hold that screen's table of seats and the rule's to each other, so neither can be edited alone.

## One run judges many changes

The second decision was about how big a landing may be. ADR 0101 had made integration land one pull request per run of the required check, and left batching to a person. The owner made that decision: the landing seat lands **combined tops**.

A top is the current master with the heads of every pull request that conflicts with nothing in authored files, which a chain of `git merge-tree` from master decides. The derived files are generated once, after the merges, instead of once per pull request. One validation run judges every member. A member that turns the run red is dropped and fixed on its own branch. The top lands by one merge, and the forge then marks every pull request it contains as merged. The decision puts the arithmetic in one line: a top of twenty members costs one run.

What it does not do is relax anything. The merge is `gh pr merge <n> --admin --merge --match-head-commit <sha>`, and it is allowed only when that exact head's `ci` concluded success and the head contains the current `origin/master`. The administrator's override is spent on a tree that was judged, never to skip a judgement. No hook or gate is disabled to land, `--no-verify` is never used, and a tag is pushed only on a commit whose `ci` succeeded.

## Nobody waits to be asked

The third decision was about stale work. A checkout or a branch that has fallen behind master is brought current by whoever finds it, without asking anyone. What it holds is preserved first: uncommitted work goes into a commit that nothing references yet (`git stash create`) and is pinned under `refs/rescue/`, so bringing the tree current cannot lose it.

A branch that nobody else has fetched may be reset or rebased. A branch that has been pushed is brought current by merging master into it, never by rewriting it. The decision gives the reason: a force-push is fast for the session that does it, and it destroys whatever another session pushed in the meantime. The rescue ref of the session that rewrote the branch cannot bring that back, and a merge costs one commit.

## Release whenever there is something to release

The fourth decision was about time. The release seat does not wait for a milestone. It cuts a release whenever master holds landed work past the last tag, and verifies what the published site serves afterwards. In the three days after that morning, three releases were recorded: 0.28.0 on 17 October at 04:25 UTC, 0.29.0 the same day at 19:47, and 0.30.0 on 18 October at 17:27.

## What is not done

[ADR 0167](@/adrs/adr-0167.md) is still `proposed`. The practice is in force anyway, because the rule that carries it is active and blocking. Its tests are cases 3150, 2999 and 720.

The landing seat assembles tops by its own procedure, not through a capability of the repository. The typed `prs compose`, which would record a top's members as a manifest, never landed. Until it does, the record of a top is its pull request: its body names the members and their heads, and its merge commit's parents are master and the top. The executor that ADR 0101 built still lands one ready pull request at a time and never passes `--admin`. A combined top is the landing seat's act, not the executor's.

The decision also named three enforcement points as other workers' work: a new worktree starts from the remote trunk rather than a stale local master, `start` makes sure the checkout's server runs before a task begins, and a checkout that has fallen behind is detected and brought current. The rule names each one as it lands.

The manual is [`docs/INTEGRATION.md`](@/docs/integration.md), which points at the rule rather than restating it. The mesh that makes a claim worth holding is described in [`docs/MESH.md`](@/docs/mesh.md).
{% endraw %}
