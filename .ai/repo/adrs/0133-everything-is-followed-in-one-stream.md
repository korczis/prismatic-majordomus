---
schema: adr/v1
id: adr-0133
kind: adr
title: Everything that happens in the repository is followed in one stream, folded and addressed
status: proposed
date: 2026-10-10
tags:
  - monitor
  - mesh
  - cooperation
  - observability
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
  - file:.ai/repo/adrs/0067-mesh-cooperation-is-authenticated-links-and-one-replicated-journal.md
  - file:.ai/repo/project/milestones/everything-is-followed-in-one-stream.yaml
  - file:apps/majordomus-cli/src/monitor/mod.rs
  - file:docs/MONITOR.md
---

# 133. Everything that happens in the repository is followed in one stream, folded and addressed

## Context

A worker that wants to know what is happening around it reads six surfaces, each with its own
cursor: the mesh journal, this checkout's board, the executor's trail, the executions, and
GitHub's pull requests and checks through `gh`. Nothing joins them. Two of them are mostly
noise: the server re-projects every attached board peer as a mesh session on each heartbeat,
so in a sample of the journal on 2026-10-10, 954 of 1000 events were `session_opened` for a
session whose content had not changed. And nothing says which of what happened is addressed to
the reader. A review asked of a session, a handover published for it, a claim that collides
with its own: these sit in the journal beside a thousand re-projections.

So sessions hand-roll the same loop: `jq` over `majordomus mesh events`, a set of sessions
already seen, `gh pr list` for merges, `git ls-remote` for a ref they wait on. Sessions missed
reviews and handovers addressed to them for hours, and one missed a remote session's messages
for a day, because the surface they read did not show them.

## Decision

One read capability, `monitor`, answers a page of **signals** after an opaque cursor.

- **A signal** is a typed line: when, which source, what kind, how urgent (`info`,
  `attention`, `action`), who did it (machine, session, client), what it is about (pull
  request, branch, paths, issue), a one-line summary, and whether it is **for you**.
- **The sources** are the records the server already keeps: the mesh journal, the board, the
  executor trail, the executions; and GitHub when it is reachable. The monitor writes nothing
  and keeps no store of its own. Every signal is derived from a record that already exists, so
  there is nothing to keep consistent.
- **The cursor** encodes each source's own position (a Lamport stamp, a sequence, a
  watermark) in one URL-safe token. A page after a cursor repeats nothing, and reconnecting is
  asking again with the last cursor.
- **The fold** keeps a session's opening only when the session is new or what it says about
  itself changed. A re-projection is counted as `suppressed` and never shown.
- **For you**: a signal is addressed to the reader when its reviewer, its handover target,
  its conflicting claim or its text names the reader's worker, role, session or checkout. A
  briefing lists those first.
- **An absent source is an answer.** A source that cannot be read is reported with its state
  and reason, never as an empty list: silence must mean that nothing happened, not that
  nothing was asked.

It is projected like every capability (ADR 0004): `majordomus_monitor` over MCP,
`GET /api/v1/monitor` over HTTP, `majordomus monitor` on the command line. The command adds
`--follow`, plus `--count` and `--until` so that a script or an agent can wait for one event
and exit. The Cockpit shows the stream live at `/cockpit/monitor`. The session-start briefing
and `majordomus context` print what happened since the last look, and the provider bootstrap
names the command, so every session uses it without being told.

## Consequences

- Following the repository costs one command, and the hand-rolled loops can be deleted.
- The fold and the addressing are the feature. A stream that shows everything is as unread as
  six streams that each show their own; a new source therefore owes a fold rule for its own
  noise.
- The monitor is observation, not authority. It never acts on a signal. A merge, a claim or an
  answer stays with the capability that owns the act.
- A notifier (Slack, mail, a push) is later a subscriber to this stream, not a seventh loop.
