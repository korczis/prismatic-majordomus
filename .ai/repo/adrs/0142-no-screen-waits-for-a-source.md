---
schema: adr/v1
id: adr-0142
kind: adr
title: "No screen waits for a source: a page shows the latest reading with its age, and the reading catches up behind it"
status: proposed
date: 2026-10-12
tags:
  - architecture
  - interface
  - ui
  - performance
  - cache
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
  - file:.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md
  - file:.ai/repo/adrs/0088-a-dashboard-is-a-projection-of-capabilities-and-every-card-names-its-source.md
  - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
  - file:apps/majordomus-cli/src/capability/readings.rs
  - file:apps/majordomus-cli/src/capability/executor.rs
  - file:apps/majordomus-cli/src/ui/source.rs
  - file:docs/MAJORDOMUS_UI.md
---

# 142. No screen waits for a source: a page shows the latest reading with its age, and the reading catches up behind it

## Context

`majordomus ui` is the surface a person is sent to first (ADR 0135). The owner said of it on
2026-10-12, after the caching work of the UI-performance batch had been measured: it "is
tragically slow", it is not built on a top-notch architecture, and it is to be solved
"architecturally, streamed", derived, enforced through `.ai`, and "blazing fast". The owner's
UI-performance pack had already set the budgets: a cached capability answers in under 50 ms at
p95, a warm server-rendered page in under 100 ms, a live update in under 250 ms.

Measured that day on the batch's own build (`perf/the-ui-answers-at-once` at `c66f49e406`, a
debug build on a machine at load 20; a release build is three to five times faster, and the
shape is the same):

| Route | Every request | First request after a start or a change |
|---|---|---|
| `/ui` | 2.2-2.5 s | 2.3 s |
| `/ui/roadmap` | 0.08-0.12 s | 8.1 s |
| `/ui/evidence` | 0.07-0.09 s | 5.6 s |
| `/ui/governance` | 0.07-0.08 s | 6.0 s |
| `/ui/panels/delivery` | not measured | 75.7 s |
| `/cockpit` | 6.1-6.3 s | 7.0 s |

The batch's cache works where it applies. Three things remain, and no cache removes them:

- **A page costs its slowest source.** `health.report` runs probes and may not be cached for
  longer than a few seconds. It is in the dashboard's first paint, so every reader of `/ui`
  waits for it.
- **A cache is empty exactly when a person looks.** An entry is keyed by the state of the
  repository (`CachePolicy::Generation`, `Checkout`). This repository changes all day, so
  the first reader after each change pays the whole cost. `project.cache-is-invisible`
  forbids the cache from answering with anything else.
- **The server cannot hold a connection open cheaply.** Each request runs on a thread of its
  own, at most 64 at once, and a response is written whole with its length
  (`http/server.rs`). The UI therefore polls every minute. A stream per open page would use
  up the handlers that requests need.

The first two are one defect: the request computes. The cache was asked to hide a
computation that should not be on the request's path at all.

## Decision

**A screen never waits for a source.** It shows the latest reading of each source, at once,
with how old it is, and the reading catches up behind the reader. Four parts, each its own
issue.

1. **The latest reading, beside the cache (I2486).** The executor keeps the last thing each
   source answered and when (`capability::readings`). A page asks for a *reading*, never an
   answer.
   - A source read in the repository as it is now is handed over from memory. No handler, no
     git and no file is touched on the request's path.
   - A reading older than a second is read again by a worker behind the caller.
   - When the repository has moved, the caller waits at most 50 ms for the new reading. Past
     that it gets the old one, marked stale and "reading again". Every other source read
     lately is read again at the same moment.
   - A source never read is waited for, because there is nothing to show instead, and is
     said to be pending past a minute.
   - A source whose answer the executor's cache already holds for the moved repository is
     current at once, without a worker.
   - The repository's own reload is not waited for either. A page is drawn from the
     generation that exists, says that the repository changed, and is read again once the
     next generation has loaded behind it (`Live::view_without_waiting`). `Live::view`, which
     every answer uses, still waits for the generation the caller saw.
   - `execute` is untouched. An answer over the API, MCP or the command line is still exact,
     and `project.cache-is-invisible` still holds. A reading is a different thing and always
     says what it is.
   - The first reader of the UI starts one background pass over `SCREENS` and `PANELS`, the
     lists the navigation is already held to, so the next screen is ready before it is
     opened. No second list of sources exists.
2. **A streamed page and one stream of changes (I2487).** The shell and every panel whose
   reading is at hand are sent at once. A panel whose first reading is pending arrives on
   the same response when it is ready, in document order, so the page is complete without a
   script. After that, one WebSocket subscription on the event model of ADR 0033 tells the
   page which readings changed, and the page replaces those panels. The minute poll and the
   catch-up re-read of part 1 go.
3. **One HTTP stack, chosen by measurement (I2488).** Long-lived streams are what the
   current stack cannot hold. The same routes, one streamed route and several hundred idle
   subscribers are measured on `tiny_http` and on `axum` over `tokio`, through the bench
   harness, on an idle host. The result is a decision record of its own. The repository
   keeps one HTTP stack, never two.
4. **The budgets are a gate (I2489).** The owner's budgets become policy keys, the bench
   measures every screen against them, and CI refuses a screen over its budget. A scan
   refuses a screen that calls a handler instead of asking for a reading.

## Consequences

- A reader may see a value from before the last change for a moment. It says so: its source
  line reads "read 12 s ago, before the repository changed; reading again", and the page
  replaces it when the new reading lands. A stale value is never drawn as current.
- A source line gains a state. A current reading older than five seconds says its age.
- The UI's sources are read again whenever the repository moves, whether or not anyone is
  looking, for five minutes after the last reader. That is background work on up to four
  threads, bounded by what readers actually opened.
- `MAJORDOMUS_CAPABILITY_CACHE=off` makes a reading run its handler and wait, like every
  other call. `MAJORDOMUS_READING_PATIENCE_MS=wait` keeps the store and makes every read wait
  for a current reading, for a test that changes a repository and then reads a page.
- The Cockpit's pages still compute on the request. Moving them to readings is the same
  change and follows the UI's.

## Alternatives refused

- **A longer-lived cache.** It would answer with a value from another state of the
  repository and call it the answer, which `project.cache-is-invisible` refuses.
- **Changing the HTTP stack first.** The measured cost is in the handlers, not the transport:
  a cached capability already answers in under 6 ms at p95. A faster transport would deliver
  the same eight-second wait.
- **A client-side application over the JSON API.** The page would no longer be complete
  without a script, a second rendering of every panel would exist, and ADR 0012 refuses a
  surface that is an application instead of a projection.

## Status

Proposed by session p12 on 2026-10-12 on the owner's words, agreed with p8, who holds the
UI-performance pack. Part 1 is implemented with this record. Acceptance is the owner's act.
