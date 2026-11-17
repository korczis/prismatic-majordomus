# The monitor

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

<!-- majordomus:unrun they follow a running server's stream, and --follow does not end on its own; cases 1050 and 1051 run these commands against a server with a declared mesh -->
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

Kinds, by source:

- **mesh**: `session_joined`, `session_changed`, `session_left`, `claimed`, `released`,
  `handover`, `handover_taken`, `review_requested`, `review_answered`, `review_withdrawn`.
- **executor**: `merged`, `merge_failed`, `verification_failed`, `refreshed`,
  `refresh_failed`, `closed`, `close_failed`, `repaired`, `repair_refused`, `lane_taken`,
  `lane_released`, `observe_failed`.
- **github**: `pr_opened`, `pr_merged`, `pr_closed`; `run_failed`, `run_cancelled` and
  `run_succeeded`, or `deploy_*` and `release_*` when the workflow's name says so. A cancelled
  run is shown, never folded: on the trunk it asks for attention, on a branch it is news. A
  failed deploy run whose `deploy` job succeeded is `deploy_needs_help`, naming the jobs that
  failed: the site went out, and something around it asks for a person.

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

- a review names you as a word in its subject or body, or is addressed to this runtime;
- a review you were asked for or answered is withdrawn;
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
`--cursor-file FILE` keeps the cursor on disk: the command starts after the cursor the file
holds and replaces it whole after every page, so a watcher that is restarted repeats nothing.
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
                   [--limit N] [--after CURSOR] [--cursor-file FILE] [--match TEXT]
                   [--count N] [--until KIND]
                   [--timeout 1h] [--interval SECONDS] [--format text|json]
```

- `--follow` asks again every `--interval` seconds (default 2) with the last cursor and prints
  only what is new. A missing or restarted server is reported once, and the follow goes on.
- `--cursor-file FILE` starts after the cursor the file holds (unless `--after` is given) and
  writes each page's cursor back. A refused cursor names the file it came from.
- `--count N` ends a follow after N signals.
- `--until KIND` ends a follow at the first signal of that kind.
- `--timeout` gives up with exit 10.
- `--match` keeps only the signals whose summary contains a text.
- `--format json` prints the page as the capability answered it. While following, it prints
  one signal per line (NDJSON).
- Durations are seconds, or a number with `s`, `m`, `h` or `d`.
- Exit codes: 0 when answered, or when a follow ends by `--count` or `--until`; 10 when the
  input is refused (a bad cursor, an unknown source) or a follow reaches `--timeout`.

## Waiting on the steps of a job

`majordomus monitor steps` waits on one thing the stream does not carry: the steps of a job of
one workflow run. A landing gated on "the hosted job's steps 4-12 are success" used to be
watched by hand-written `gh api` loops, and those loops broke in three ways. This command reads
the run and its jobs through `gh api` and decides each step the way those loops got wrong
(I4624).

```sh
majordomus monitor steps --run <run-id> --job '<job name>' --steps 4-12
majordomus monitor steps --branch <branch> --sha <sha> --workflow <name> --job '<job>' --steps <N-M>
```

How each step of the range is read:

- **Pending** means the step has not concluded: its status is not `completed`, or its
  conclusion is empty. `gh run view --json jobs` prints `""` for a running step and the REST
  API prints `null`; both are pending. A loop that took `""` for done reported green after
  four of nine steps.
- **Red** means the step concluded anything but `success`: `failure`, `cancelled` (a run
  cancelled by concurrency, or cut at its timeout), `timed_out`, and `skipped` unless
  `--allow-skipped`. The first red step ends the wait at once, even while other steps run.
- **Green** means every step of the range, in every job whose name matches `--job`, concluded
  `success`. `*` in `--job` and `--workflow` matches any run of characters, over the whole name.

What a finished run settles:

- While the run is going on, a job not listed yet is pending, and so is a step past the job's
  last listed step.
- Once the run or the job has finished, both are red. The reason names the steps the job does
  list (`it lists steps 1-7, 13-15`), because step numbers can have gaps: post steps are
  numbered apart.

Polling is gentle:

- `--interval` defaults to 180 s and is refused below 120 s. On 2026-11-15 several watchers
  polling every 60 s spent an account's core API limit.
- Each poll costs two calls: the run (or the branch's runs for the head) and its jobs, up to
  100 jobs.
- A rate-limited answer (403 or 429 with no calls left, or `Retry-After`) waits until the
  limit resets. So does a success that leaves two calls or fewer.
- A transient failure (a 5xx, a dropped connection, a `gh` call past 30 s) doubles the wait,
  up to 30 minutes.

Other behaviour:

- `--branch` needs `--sha`. The newest run for that head is read. When several workflows ran
  for the head, `--workflow` must say which one, or the command refuses.
- `--github OWNER/NAME` reads another repository. By default, `gh` resolves the repository
  from the checkout's remotes.
- Each poll that decides nothing prints one line on stderr. The end prints the run, each
  matching job, each step's verdict and the reason, as text or, with `--format json`, as one
  document.
- Exit codes:
  - 0 green;
  - 10 red;
  - 11 when `--timeout` (default `2h`) passes while pending;
  - 12 when GitHub cannot be read and asking again will not help: no `gh`, no repository,
    no such run, several workflows and none named.

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
  - `src/commands/monitor.rs` covers rendering and the request target, and how `monitor
    steps` is dispatched;
  - `src/monitor/steps.rs` covers the step verdicts and the polling, over `gh api` answers
    recorded from this repository's runs (`apps/majordomus-cli/tests/fixtures/gh-api-steps/`):
    an empty-string conclusion, a cancelled step, a missing job, a range past the job's steps,
    green, a rate-limited answer and the deadline;
  - `src/cockpit/monitor.rs` covers the row.
- Cases:
  - `1050_the_monitor_folds_the_noise`: one server, the fold, for-you, refusals and resuming;
  - `1051_a_followed_stream_can_be_waited_on`: `--count`, `--until`, `--match`, `--timeout`
    and NDJSON;
  - `1052_an_absent_source_is_reported`: GitHub through a fixture `gh`.
