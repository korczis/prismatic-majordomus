+++
title = "A session is current, attached, announced and useful before it changes anything"
description = "A session is current, attached, announced and useful before it changes anything"
weight = 72
[extra]
kind = "rule"
slug = "project-a-session-joins-the-cluster-before-it-works-1"
identity = "project.a-session-joins-the-cluster-before-it-works@1"
status = "active"
source = ".ai/repo/rules/project/a-session-joins-the-cluster-before-it-works.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-13 a session spent three hours as the only worker of a repository that had
nineteen. Its checkout was ninety commits behind the trunk, two releases had been tagged
without it, it had never announced itself, and the cluster's open asks to everybody went
unanswered while it looked for work of its own. The prose that would have prevented all of
it was in `CLAUDE.md` and was read. See ADR 0156 for the account and the decision.

## Required behaviour

1. **Current.** Run a bounded fetch and compare with the remote trunk before you build on
   the checkout. Report how many commits you are behind and the newest release tag. "Up to
   date" is a claim about a fetch you just made; a hook line from the start of the session
   is not one. An unreachable remote is `unverified`.
2. **Attached.** If `majordomus_*` tools are unavailable, say so at once, on the board and
   to the person, and continue on the CLI. A silent fall-back is how a session becomes
   invisible.
3. **Announced.** Open a session on the live server (`majordomus mesh session open`, or
   `majordomus_announce`) with your intent, and claim the paths and the identifiers —
   an ADR number, a case number, an issue id — before you write them
   (`project.work-is-claimed-before-it-is-built`). Re-announce after a reconnect: an
   announcement belongs to a connection.
4. **Useful.** Read the board (`majordomus mesh peers`, `mesh state --full`) before you
   choose work. Asks to all, review requests with no answer and tasks no peer holds come
   before work you invented. Take one, or write on the board why none is yours. Where a
   derive is routed to another machine, route yours there; do not build on a host that is
   not the lane.

## Verification

Advisory until the capability that derives the four facts lands (ADR 0156). Then this rule
is raised to `blocking` as version 2, `start`, `check`, `finish` and the commit and push
hooks refuse a change from a session that is not announced or is behind the trunk by more
than the policy's bound, and `test/cases/3010_a_session_joins_the_cluster.sh` proves the
refusals and the unreachable-remote case.
{% endraw %}
