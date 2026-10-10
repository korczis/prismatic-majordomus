+++
title = "The monitor"
description = "the monitor: everything that happens around a repository in one stream — mesh sessions, claims, handovers and reviews, the executor's outcomes, GitHub's pull requests and runs — with the noise folded away, what names you first, one cursor across every source, and the command, MCP, HTTP and Cockpit surfaces"
weight = 58
[extra]
source = "docs/MONITOR.md"
+++

{% raw %}

`majordomus monitor` is everything that happens around a repository, in one stream: sessions,
claims, handovers and reviews on every linked machine, the executor's merges and failures, and
GitHub's pull requests and workflow runs. What names you comes first, and the noise is folded
away and counted. The decision is ADR 0133
(`.ai/repo/adrs/0133-everything-is-followed-in-one-stream.md`).

## Why it exists

A worker that wanted to know what was happening read several surfaces, each with its own
cursor:

- the mesh journal (`majordomus mesh events`);
- this checkout's board (`majordomus_peers`);
- the executor's trail (`majordomus prs events`);
- GitHub, through `gh pr list` and `gh run list`.

Most of the journal is noise. The server re-projects every attached board peer as a mesh session
on each heartbeat: in a sample of 1000 events on 2026-10-10, 954 were a session saying again
what it had already said. Nothing said which events were addressed to the reader. Sessions missed
reviews and handovers asked of them for hours, and each one hand-rolled the same `jq` loop.

## Quick start

```sh
majordomus monitor                         # the newest signals of the last hour
majordomus monitor --for 1b,cd             # ... with what names 1b or cd listed first
majordomus monitor --follow                # keep printing what is new
majordomus monitor --follow --match '#885' --until merged --timeout 1h
                                           # wait for one pull request to merge, then go on
```

Set `MAJORDOMUS_MONITOR_FOR` once instead of passing `--for` every time. The session-start
briefing and `majordomus context` use the same variable for their monitor lines.

The stream lives in the shared server, because the mesh journal does, so the command asks this
checkout's running server. With no server, the command prints the reason and exits 0;
`majordomus serve ensure` starts one.

## What a signal is

<div class="overflow-x-auto" tabindex="0">

| field | meaning |
|---|---|
| `at` | when it happened, in Unix seconds, on the clock of whoever recorded it |
| `source` | `mesh`, `executor` or `github` |
| `kind` | what happened (below) |
| `severity` | `info`; `attention` when something failed or waits; `action` when it waits for you |
| `machine`, `session`, `actor` | where and by whom; a machine by its mesh display name when known |
| `pr`, `branch`, `issue`, `paths` | what it is about |
| `summary` | one line, at most 200 characters |
| `for_you` | whether it names the reader |
| `link` | where to read more, when there is a page for it |

</div>


Kinds, by source:

- **mesh**: `session_joined`, `session_changed`, `session_left`, `claimed`, `released`,
  `handover`, `handover_taken`, `review_requested`, `review_answered`.
- **executor**: `merged`, `merge_failed`, `verification_failed`, `refreshed`,
  `refresh_failed`, `closed`, `close_failed`, `repaired`, `repair_refused`, `lane_taken`,
  `lane_released`, `observe_failed`.
- **github**: `pr_opened`, `pr_merged`, `pr_closed`; `run_failed` and `run_succeeded`, or
  `deploy_*` and `release_*` when the workflow's name says so.

## The fold: what is not shown

The fold is the feature. A stream that shows everything is as unread as six streams that each
show their own. Every record is read; a record that is not news is counted in `suppressed`.

- **A session** is shown when it first says what it does, and again only when its worker,
  intent, task, issue, milestone, branch or client changes. A head that moves is not news.
- **A connection that never says what it does** is not news. Its arrival and its departure are
  both suppressed. This covers a client that reconnects every minute with no intent.
- **A claim** is not news when its session already holds the same claim (same scope, intent
  and mode).
- **A release** of a claim nobody saw taken is not news.
- **An executor step that decided nothing** is not news: an observation, a selection, an
  attempt before its outcome, an idle tick.
- **A green run** on a branch other than `master` is not news; a pull request's CI passing is
  the expected outcome. A failed run is always news, and so is a green run on `master`
  (master's checks, a deploy, a release).

## For you

A signal is `for_you` when any of these holds:

- a review names you as a word in its subject, or is addressed to this runtime;
- an answer replies to a review you asked for;
- a handover's body or branch names you;
- a claim's intent names you;
- an executor event's detail names you;
- a pull request's title or branch names you;
- a failed run is on a branch that names you.

Names are matched as whole words, case-insensitively: `1b` matches "ask 1b" and
`fix/1b-hello`, but not `p1b`.

An MCP caller is also its own board session (`board-<peer>`). A text page lists the signals for
you first, under `For you (N)`.

## The cursor

Every page ends with a `cursor`. Asking again with `after` set to it returns only what came
after, and nothing on the previous page comes back. The cursor carries each source's own
position:

- for the mesh, the full causal key (Lamport stamp, stream, sequence), so that events sharing
  a stamp are never split by a page boundary;
- for the executor, how many lines of its trail were read;
- for GitHub, the last second read. Signals of one second are never split across pages.

A cursor that cannot be read is refused with exit 10; it never restarts the stream silently.
Without a cursor, a page holds the newest `limit` signals of the last `since` seconds, and its
cursor stands after everything read.

## Sources and their states

Every page lists every source with its state:

- `ok`;
- `absent`, with a reason: the mesh is not running, the executor has recorded nothing, `gh`
  could not be started;
- `error`, with a reason: `gh` failed or took longer than its budget;
- `skipped`: not asked for in `sources`.

**An absent source is an answer, never an empty list.** Silence must mean that nothing
happened, not that nothing was asked.

GitHub is the network. One fetch reads the 40 newest pull requests and runs. It is bounded to
4 seconds per `gh` call and shared by every reader for 30 seconds. The session-start briefing
and `majordomus context` read only the local sources.

Executions (`executions.events`) are not a source yet. Their sequences belong to one execution
of one process, and nothing joins them into one feed.

## The command

```
majordomus monitor [--follow] [--for NAMES] [--sources mesh,executor,github] [--since 30m]
                   [--limit N] [--after CURSOR] [--match TEXT] [--count N] [--until KIND]
                   [--timeout 1h] [--interval SECONDS] [--format text|json]
```

- `--follow` asks again every `--interval` seconds (default 2) with the last cursor and prints
  only what is new. A missing or restarted server is reported once, and the follow goes on.
- `--count N` ends a follow after N signals.
- `--until KIND` ends a follow at the first signal of that kind.
- `--timeout` gives up with exit 10.
- `--match` keeps only the signals whose summary contains a text.
- `--format json` prints the page as the capability answered it. While following, it prints
  one signal per line (NDJSON).
- Durations are seconds, or a number with `s`, `m`, `h` or `d`.
- Exit codes: 0 when answered, or when a follow ends by `--count` or `--until`; 10 when the
  input is refused (a bad cursor, an unknown source) or a follow reaches `--timeout`.

## Other surfaces

- MCP: `majordomus_monitor`, with the same input (`after`, `limit`, `sources`, `for`,
  `since`). An agent's own board session counts as one of its names.
- HTTP: `GET /api/v1/monitor?after=&limit=&sources=&for=&since=`. The OpenAPI document
  describes `MonitorPage` and `MonitorSignal`.
- Cockpit: `/cockpit/monitor[?for=NAMES]` shows the newest signals and, while visible, adds
  what is new on top every five seconds. The overview and the mesh page link it.
- Session start: the briefing prints a `Monitor:` line under `Mesh:`, giving the count of the
  last hour, the signals for you, and those that ask for attention.
- `majordomus context`: the `WHAT HAPPENED` section carries the same lines.

## What it is not

- It writes nothing and keeps no store. Every signal is derived from a record that already
  exists.
- It is observation, not authority. A merge, a claim or an answer stays with the capability
  that owns the act.
- It is not a notifier. Slack, mail or push would later subscribe to this stream rather than
  add a seventh loop.

## Proof

- Unit tests:
  - `src/monitor` covers the cursor, the merge across sources, the fold of each source and
    for-you;
  - `src/commands/monitor.rs` covers rendering and the request target;
  - `src/cockpit/monitor.rs` covers the row.
- Cases:
  - `1050_the_monitor_folds_the_noise`: one server, the fold, for-you, refusals and resuming;
  - `1051_a_followed_stream_can_be_waited_on`: `--count`, `--until`, `--match`, `--timeout`
    and NDJSON;
  - `1052_an_absent_source_is_reported`: GitHub through a fixture `gh`.
{% endraw %}
