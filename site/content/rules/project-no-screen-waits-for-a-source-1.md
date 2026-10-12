+++
title = "A screen shows the latest reading of its sources at once, and never waits for a handler"
description = "A screen shows the latest reading of its sources at once, and never waits for a handler"
weight = 127
[extra]
kind = "rule"
slug = "project-no-screen-waits-for-a-source-1"
identity = "project.no-screen-waits-for-a-source@1"
status = "active"
source = ".ai/repo/rules/project/no-screen-waits-for-a-source.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-12 the owner called the UI "tragically slow" a day after it became the primary
surface. The measurements were plain. `/ui` took 2.2 to 2.5 s on every request, because
`health.report` runs probes and was part of the first paint. The first request after any
change of the repository took 5.6 to 8.1 s on three screens, because a cache keyed by the
repository's state is empty exactly then. One panel took 75.7 s. A cache had been added the
same day and worked wherever it applied.

The defect was not the cache. A page computed while a person waited, and a page costs its
slowest source. `project.cache-is-invisible` rightly forbids a cache from answering with a
value from another state of the repository, so no cache could close the gap. ADR 0142 takes
the computation off the request instead.

## Required behaviour

1. **A screen asks for readings.** Every source of a screen is read through
   `ui::source::ask`, `ask_all` or `ask_input`, which ask `Context::read`. Outside
   `ui/source.rs` and tests, nothing under `apps/majordomus-cli/src/ui` calls `execute`.
2. **A known source costs a lookup.** A source read in the repository as it is now is
   handed over from memory. Its handler runs on a worker of `capability::readings`, behind
   the caller.
3. **A moved repository is not waited for.** When the repository has moved, a caller waits
   for a new reading no longer than its patience (50 ms for a page) and is then handed the
   reading it has, marked stale.
4. **A reload is not waited for.** A screen is drawn from the generation the server has
   (`Live::view_without_waiting`). When the repository has moved past it, the screen says
   so and the next generation loads behind the reader.
5. **A reading says how it stands.** A stale or pending reading is shown as that, and a
   stale one with its age. Its source line carries `data-catching-up` until a current
   reading replaces it. A current reading older than five seconds says its age.
6. **An answer stays exact.** `execute` runs the handler or returns what the handler would
   return. Nothing a reading does changes what the API, MCP or the command line answer.
7. **It can be switched off.** `MAJORDOMUS_CAPABILITY_CACHE=off` makes a reading run its
   handler and wait for it.

## Failure behaviour

`test/cases/1096_no_screen_waits_for_a_source.sh` fails when a file under
`apps/majordomus-cli/src/ui` other than `source.rs` calls `execute` outside a test module,
when a served screen waits for its sources after the repository changed, or when a screen
that is catching up does not say so. The readings tests in
`apps/majordomus-cli/tests/executor.rs` fail when a known source is waited for, when a moved
repository is waited for past the patience, when two callers of one source run two
handlers, or when a failing or panicking source leaves no reading.

## Verification

```bash
MJ_TEST_JOBS=2 bash test/run.sh 1096_no_screen_waits_for_a_source
cd apps/majordomus-cli && cargo test --test executor
```

The latency budgets a screen is held to, and the gate that measures them, are I2489.
{% endraw %}
