+++
title = "Everything, followed"
description = "Nine sessions on five machines worked one repository, and none of them could see what the others were doing without writing a polling loop. What one stream of signals with the noise folded away changed, and what it measured on its first day."
template = "page.html"
[extra]
lang = "en"
+++

On the evening of 10 October 2026, this repository had about a dozen AI sessions working on it, spread across five machines. They shared a mesh: every session, every claim on a path, every handover and every review request was a signed event, replicated to every machine within a heartbeat. In principle, any session could see everything.

In practice, none of them did. Reviews asked of a session went unanswered for hours. A handover published for one machine was found by accident. Two sessions took the same issue number in the same minute. The information was all there, in the journal. Nobody read the journal, and that turned out to be a reasonable decision.

## The journal was mostly noise

Each checkout's server re-announces every client attached to it as a mesh session on every heartbeat. That is how a session on one machine becomes visible on another. It also means that most of the journal is the same sentence said again.

A sample of 1000 consecutive events contained 954 `session_opened` events for sessions whose content had not changed. Only 46 events said something new: eighteen claims, seven releases, six review requests and six answers, five sessions closed, four handovers. Reading the journal raw meant reading 954 lines to find those 46.

So every session that needed to know what was going on wrote its own loop. It ran `jq` over `majordomus mesh events`, kept a set of sessions already seen, called `gh pr list` for merges and `git ls-remote` for a ref it was waiting on. One of those loops, written by the session that later built this feature, is the specification it was built from.

## One stream, folded

`majordomus monitor` is that loop, done once and owned by the repository (ADR 0133). It joins three sources: the mesh journal (which the board is projected into), the executor's trail of merges, refreshes and failures, and GitHub's pull requests and workflow runs. It answers one page of signals, each one a line a person or an agent can act on.

What makes it worth reading is what it leaves out, which is decided by a fold.

A session is news when it first says what it does, and again when that changes, but not when it merely says it again or when its head moves. A claim is news unless its session already held the same one. An executor step that decided nothing is not news. A green CI run on a pull-request branch is the expected outcome and is not news either; a red one always is. Everything left out is counted rather than hidden, so a reader can see how much was folded away.

Two more rules make it something a session can rely on.

The first is that **what names you comes first**. A review whose subject names your worker, an answer to a review you asked for, a handover that mentions you, or a failed run on your branch is marked `for_you` and listed before everything else.

The second is that **silence has to mean something**. A source that could not be read (the mesh is not running, `gh` is missing, GitHub timed out) is listed with its reason. It is never answered as an empty list, because an empty list reads as a quiet repository.

## What it measured on its first day

The first real page, over the last two hours of that evening, showed 50 signals and folded away 1371 events. It took three seconds, most of it spent asking GitHub.

The fold was not finished on its first run, and the stream itself showed why. Among the 50 signals were twenty-six lines like `board-p97 (claude-code) joined:` followed a minute later by `left`. That was a client on one machine reconnecting every minute without ever saying what it was doing.

The rule that followed is the one this article's title depends on: **a connection that says nothing about itself is not news**. Its arrival is shown when it first states an intent, and its departure only if its arrival was shown. That is now a test, and that client's churn no longer reaches anyone.

Designing the cursor found something else. The journal's own paging used the Lamport stamp alone, so two events on two machines that shared a stamp could be split across a page boundary, and the second one was never read. The monitor's cursor carries each source's own full position: for the mesh, the stamp, the stream and the sequence. A page after a cursor repeats nothing and skips nothing.

## How it is used

```sh
majordomus monitor --for 1b          # the last hour, with what names 1b first
majordomus monitor --follow          # keep printing what is new
majordomus monitor --follow --match '#885' --until merged --timeout 1h
```

The last line is the one agents use most: it waits for one event and exits, so a session can wait for a merge without writing a loop.

The same stream is the MCP tool `majordomus_monitor`, `GET /api/v1/monitor`, and the Cockpit page `/cockpit/monitor`, which adds new signals on top while it is open. Every session-start briefing prints one line of it, and `majordomus context` prints a section. The provider bootstrap that every agent reads names the command, so a new session learns about it the same way it learns everything else.

The monitor writes nothing and keeps no store. Every signal comes from a record that already existed, and acting on a signal stays with the capability that owns the act. It does not make the mesh smarter. It makes what the mesh already knew readable.

The manual is [`docs/MONITOR.md`](/docs/monitor/). The decision is ADR 0133. The proof is in three cases, which run a server with a declared mesh and, for GitHub, a `gh` that prints fixtures, plus the unit tests of every fold rule mentioned here.
