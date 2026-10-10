+++
title = "Working beside other sessions"
description = "what a session does when it is not alone, and why: read the mesh as well as the board, ask for an approval where the act is taken, leave a head to the integration lane until its turn, hold pushes while a release tag has no record, and look at a lock after stopping its holder"
weight = 73
[extra]
source = "docs/WORKING_BESIDE_OTHER_SESSIONS.md"
+++

{% raw %}

Several sessions work this repository at once: in linked worktrees of one machine, on other
machines, and under different assistants. Each knows its own conversation and nothing of the
others'. The repository's layer carries a workflow for that,
`.ai/repo/workflows/working-beside-other-sessions.md`, and every bootstrap the repository
generates points a worker at it.

This page says what the workflow asks and why, for a reader who wants the reasons without the
procedure. The workflow is the text a session follows, and where the two differ the workflow
is right.

It was written after 2026-10-09 and 2026-10-10. About ten sessions on four machines shipped
two releases through one integration executor in those two days, and lost hours to five
failures. None was a disagreement about the work, and none was written anywhere a new session
would have read.

## Read the board and the mesh

The peer board shows the workers of this machine. A session on another machine, or a worker
of another assistant, can reach a session only through the mesh: an announcement that names
it, a review request, a handover ([`MESH.md`](@/docs/mesh.md)). Nothing delivers these, so a session
that reads only the board never learns that it was asked. The executor of those two days did
exactly that for a day, and a ready pull request and an order of the owner's waited in the
mesh state unread.

The workflow has a session read both the board and the mesh state when it starts and whenever
it coordinates, and answer where it was asked. Joining the mesh automatically when a session
starts is separate work and is not part of this.

## An approval is asked for where the act is taken

A person approves an act in the session they are talking to. Another session saying that the
owner approved something is a report about that session. It cannot carry that the person knew
which session would act, on what state, or with which permissions. The owner approved a merge
shortcut in one session, two sessions relayed it to the executor, and the executor, having
refused twice, reasoned its way to acting on the relays and was stopped by its permission
layer.

The workflow has the acting session ask its own person, record the question, and take the
path that needs no approval until the answer comes. The rule
`project.mesh-is-observation-not-authority` states it of everything a session reads on the
mesh or the board.

## One pull request stands between repaired and merged

The integration lane brings one pull request up to master, waits for its required check,
merges it, and only then touches the next ([`INTEGRATION.md`](@/docs/integration.md)). Every landing
regenerates derived files that every other head also regenerates, so a head brought up to
master before its turn is conflicting again after the next landing, and the derive and the
check run spent on it are lost.

The workflow has an author leave that to the lane, and names the one thing that may be
prepared ahead: the next head, merged and derived locally on top of the head about to land,
and pushed only after that one has merged.

## A release tag closes the lane until its record lands

From the push of a release tag until its record's pull request is on master, the release
check fails every other head, by decision ([`DISTRIBUTION.md`](@/docs/distribution.md) describes the
release). A push to a pull-request head in that window starts a run that cannot pass and
takes runners from the run that ends the window.

The workflow has every session hold its pushes for that window, has the releasing session
close and reopen the record's pull request as soon as it opens, because the run the pipeline
dispatches for it did not satisfy the branch protection, and has a tag pushed only from a
commit of master whose `ci` check concluded success.

## After a stop, look at the lock

A job gives its lock back on the way out, and a stop does not always let it leave that way.
The executor stopped one of its own jobs seconds after the job took the machine's derive lock,
and every other session's derive waited two hours and fifteen minutes behind a process that
no longer existed. The derive has since learned to reclaim a dead owner's lock. The workflow
keeps the habit for every lock, lease, marker and claim: after stopping a job that held or
waited for one, look at it before doing anything else, and give back only what is yours.

## Where it is held

<div class="overflow-x-auto" tabindex="0">

| what | where |
|---|---|
| the procedure a session follows | `.ai/repo/workflows/working-beside-other-sessions.md` |
| the clause about a reported approval | `.ai/repo/rules/project/mesh-is-observation-not-authority.v1.md`, clause 7 |
| the pointer every worker loads | `AGENTS.md` and `CLAUDE.md`, rendered from `.ai/repo/providers/` |
| the proof | `test/cases/1034_working_beside_other_sessions_is_written_where_it_is_read.sh` |

</div>


The case holds that the layer discovers the workflow, that every generated bootstrap leads to
it, that the rule states the clause, that every tool, command and script the workflow names
exists, and that this page keeps the workflow's five headings. It cannot hold that a session
does what it read. That is decided by review and by each session's own permission layer.
{% endraw %}
