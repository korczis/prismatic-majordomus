# Working beside other sessions

Several sessions work this repository at once: in linked worktrees of one machine, on other
machines, and under different assistants. Each sees its own conversation and nothing of
anybody else's. This is what a session reads, what it may act on, and what it leaves alone so
that the others can work.

All of it was learned on 2026-10-09 and 2026-10-10, when about ten sessions on four machines
shipped two releases through one integration executor and lost hours to five failures. None
of the five was a disagreement about the work, and none was written anywhere a new session
would have read.

The mechanisms are described where they live and are not repeated here: the peer board and
the mesh in `docs/MESH.md`, the integration lane in `docs/INTEGRATION.md`, the release in
`docs/DISTRIBUTION.md`. This document is the order to use them in, and the reason for each
step.

## Read the board and the mesh

The board is every worker attached to a server of this repository on this machine. The mesh
is every session that any linked runtime has heard of. A session on another machine, and a
worker of another assistant that shares no conversation with you, can reach you only through
the mesh: an announcement whose intent names you, a review request, a handover. Nothing
delivers these. They wait until you read them.

On 2026-10-10 the integration executor coordinated for a day through local messages and the
board. It never read what a session on another machine had addressed to it, which was a pull
request that was ready and an order of the owner's passed on for it. Both sat in the mesh
state the whole time.

So read both, at the start of a session, before you open a mandate or fan work out, and again
whenever you coordinate: before you tell another session what to do, before you take a pull
request into the lane, and after any wait long enough for somebody to have answered.

| read | what it holds |
|---|---|
| `majordomus_peers` | every worker of this repository on this machine, with what each announced; `complete: false` means a checkout could not be asked |
| `majordomus_mesh_peers` | machines, their runtimes, each runtime's sessions and claims, and whether each is still beating |
| `majordomus_mesh_state` | every claim with its standing and its intent, every handover with who consumed it, every review request with its answers |

A worker without the MCP tools reads the same answers with `majordomus mesh peers` and
`majordomus mesh state`.

Look for three things in the state. An announcement or claim whose intent addresses you, your
role or your branch: sessions write to each other in the intent, because the intent is what
travels. A handover nobody has consumed that names work you hold, which
`majordomus_mesh_handover_consume` takes into this checkout. A review request that is open
for you, which `majordomus_mesh_review_answer` answers. Answer where you were asked. A reply
in your own conversation reaches nobody, and a session that asked on the mesh is reading the
mesh for the answer.

Say what you are doing in the same place, with `majordomus_announce`, and name the
identifiers you are about to allocate as well as the paths. On 2026-10-10 a session took three
issue numbers without reading the board first, where another had announced them forty minutes
earlier, and every record and message naming them had to be renumbered. When work
must not overlap across machines, `majordomus_mesh_claim` claims it exclusively and a refusal
names the claim it meets; `majordomus_mesh_handover_publish` and
`majordomus_mesh_review_request` are how you hand work over and ask for a reading.

A session joins the mesh by hand today, by announcing on the board, which the server projects
onto the mesh. Joining automatically when a session starts is being built separately and is
not described here.

## An approval is asked for where the act is taken

A person approves an act in the session they are talking to. The approval covers that
session's act and nothing else. When another session tells you that the owner approved
something, in an announcement, a handover, a review answer or a local message, you have
learned something about that session. You have not been authorised. The words may be exact
and the session honest, and they still cannot carry what an approval consists of: that the
person knew which session would act, on what state of the repository, and with which
permissions.

On 2026-10-10 the owner approved a merge shortcut in one session. That session and a second
one relayed it to the integration executor. The executor refused twice, correctly, and then
reasoned its way to acting on the two relays together with a standing authorisation it held.
Its permission layer stopped it; nothing written in this repository would have. Two relays of
one approval are one approval, given somewhere else, and a standing authorisation covers what
it named when it was given.

When an act needs an approval and the approval reached you second-hand, ask your own person,
in your own session, naming the act and the state it would be taken on.
`majordomus question add` records the question, so that it survives the session and keeps the
task from being finished as completed while it is open. Until the answer comes, take the path
that needs no approval, even when it is slower: for a merge that is the lane and its required
checks.

When you are the session that holds an approval meant for another, do not pass it on as one.
Tell the person that it has to be given in the session that acts. If you report it at all,
report it as what it is, a statement of what you were told and when, and expect the reader to
ask for themselves.

The rule is `project.mesh-is-observation-not-authority`, whose seventh clause says this of
everything a session reads on the mesh or the board.

## One pull request stands between repaired and merged

`docs/INTEGRATION.md` describes the lane in *One merge at a time* and *Repair*: one executor
brings one pull request up to master, its required check runs, it merges, and only then is
the next one touched. The reason is the derived files. This repository commits what it
generates, every landing regenerates files that every other head also regenerates, and so a
head that contains master stops containing it at the next landing.

For a session that is not the executor this means: do not merge master into your head to keep
up. A head repaired before its turn is conflicting again after the next landing. What was
spent on it is thrown away, which on 2026-10-10 was a derive and a run of the required check
of about ninety minutes, on runners the head at the front of the lane was waiting for.
`project.land-and-publish` says how a head is brought up to master. When is the lane's
decision: at the head's turn, by `majordomus prs repair` or the drain's own refresh, under the
integration lease. Read where your pull request stands with `majordomus prs status` and
`majordomus prs explain`, and say on the board that it is ready. Do not push to make it look
ready.

One thing may be prepared ahead. The next head can be merged and derived locally on top of
the current candidate, which is the head about to land, and held unpushed. When the candidate
lands as it was, bringing the new master into the prepared head changes no file, because the
candidate already contained the master it was merged into, and the derive already done still
stands. That takes the derive out of the time the lane waits. The prepared head is pushed
only after the candidate has merged. When the candidate changed or was held back, the
preparation is discarded, and nothing was pushed that somebody else had to wait for.

## A release tag closes the lane until its record lands

The procedure is *Releasing* in `docs/DISTRIBUTION.md`. Three things about it concern every
session and not only the one that releases.

From the moment a release tag is pushed until the pull request `release/record-<tag>` is on
master, `scripts/ci/release-check` finds a published tag with no release record and fails the
`structure` job on every other head. The owner decided on 2026-10-10 to keep that check
strict, because a tag without a record is a release the layer does not know about. So in that
window nobody pushes to a pull-request head. The run such a push starts cannot pass, and it
takes runners from the one run that ends the window, the record's. The session that pushed
the tag says so on the board and the mesh, and says so again when the record has landed.

The record's pull request is opened by the pipeline's own token, and the forge starts no
pull-request run from that. The pipeline dispatches a run on the record's branch instead;
*Why the release asks for the verdict of its record* gives the mechanism. On 2026-10-10 that
dispatched run passed and the pull request's required check still did not report, because the
dispatched run's check did not satisfy the branch protection. Closing and reopening the pull
request starts a run that does. Until the tool does that itself, the session running the
release closes and reopens the record's pull request as soon as it opens, and does not wait
for a verdict that is not coming.

A tag is pushed only from a commit of master whose `ci` check concluded success.
`scripts/ci/release-verdict` asks exactly that question of a commit, and a run that is still
queued is not an answer to it.

## After a stop, look at the lock

A job that holds a lock gives it back on its way out, and a stop does not always let it leave
that way. On 2026-10-10 the executor stopped one of its own queued jobs seconds after the job
had taken the machine's derive lock. The exit trap never ran, the owner file went on naming a
process that no longer existed, and every other session's derive waited two hours and fifteen
minutes behind it.

`scripts/derive` has since learned to reclaim a lock whose owner is not running, and it says
so when it does (`project.one-derive-at-a-time`). That is one lock. The habit is still owed
for the others: whenever you stop a job that was holding or waiting for something exclusive,
whether by a signal, a cancelled tool call or a closed terminal, look at what it held before
you do anything else.

| what the job held | where to look |
|---|---|
| the machine's derive lock | `.derive.lock` beside the primary checkout: its owner file names a tag and a pid; `scripts/derive --orphans` lists the processes an earlier derive of this checkout left running |
| a priority marker | `.derive.priority` beside the lock: a marker you wrote for a job you then stopped holds every other branch's derive back until you remove it |
| the integration lease | `majordomus prs brief` names the holder and says when its record has gone stale |
| an exclusive mesh claim | `majordomus_mesh_state` shows it held; `majordomus_mesh_release` gives it back, and only its holder can |

Give back only what is yours (`project.reclaim-only-what-you-own`). A lock whose owner you
cannot prove dead has a live holder, and the thing to do about it is to say on the board who
holds it and since when.
