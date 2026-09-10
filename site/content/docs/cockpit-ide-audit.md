+++
title = "The Cockpit as a development surface: baseline, archaeology and gap matrix"
description = "forensic finding, 2026-09-10: what the Cockpit would need in order to become a development surface — the governance preflight and its two defects, the two-program split, the system run end to end with every command and response recorded, the gap matrix, the drift audit, and the phased plan with the failures that must be fixed first"
weight = 33
[extra]
source = "docs/COCKPIT_IDE_AUDIT.md"
+++

{% raw %}

A forensic finding, measured on 2026-09-10 in a linked worktree at
`feature/development-runtime-model`, branched from `origin/master` at `08e4bb252`, against
a server this worktree built and started itself. It is phase 01 of the Cockpit IDE /
control-plane pack and it implements nothing: it establishes what exists, proves it by
running it, and says what the smallest safe next phase is.

The architecture it measures against is [ADR 0040](../.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md)
and [`DEVELOPMENT_RUNTIME.md`](@/docs/development-runtime.md): one canonical runtime — the
capability registry of the Rust executable — and every surface a consumer of it. This
document is the evidence behind that decision and the plan that follows from it.

Every claim here was produced by a command, and the command is shown. Where a number
appears without a command, it is a defect in this document.

## The finding in one paragraph

The Cockpit is a faithful, read-only projection of a registry that has almost nothing
development-shaped in it. Of 78 built-in capabilities exactly three can mutate anything,
and the OpenAPI document the server serves has exactly three non-GET operations to match —
`executions.start`, `executions.cancel`, `peers.announce`. Every command of the development
lifecycle lives in a second program, `bin/majordomus`, and all 28 of its nodes in the
command graph are withheld from every machine surface. So the Cockpit cannot be made an IDE
by adding pages: there is nothing behind the pages to call. The first phase of work is not
UI, it is giving the runtime a mutating development surface for the Cockpit to consume.

Three further defects are load-bearing and were found by running the system rather than
reading it: `majordomus run` executes in its own process and is invisible to the running
server, so the execution plane the Cockpit shows and the execution plane a person uses are
disjoint; a server serving code that no longer exists on disk reports itself `ready`; and
`/cockpit/worktrees` takes 18.3 s where every other page takes 1–4 ms.

## 1. Governance preflight

The pack requires the repository's own mechanisms to be exercised, and any that are broken
to be recorded rather than routed around.

<div class="overflow-x-auto" tabindex="0">

| Step | Command | Result |
|---|---|---|
| contract | read `README.md`, `AGENTS.md`, `CLAUDE.md`, `.ai/README.md` | ok — `AGENTS.md` carries its generated stamp from `.ai/repo/policy.yaml` |
| effective rules | `majordomus rules list --json` | ok — 120 effective at the time of the audit; 121 after this branch's rule |
| context | `majordomus context` | ok — 18 of 300 lines, `working_tree clean`, no active task at that moment |
| scoped context | `majordomus context resolve docs/DEVELOPMENT_RUNTIME.md` | ok — resolves to `ai.layer` alone; `docs/` is outside `.ai/` and gets the root chain only |
| worktree topology | `majordomus-cli worktree` | ok — `canonical`, container `…-wt`, trunk `master` |
| collision check | `scripts/collision-check --new <paths>` | ok — nothing else claims `docs/DEVELOPMENT_RUNTIME.md`, `docs/COCKPIT_IDE_AUDIT.md`, the ADR or the rule |
| task lifecycle | `majordomus start "…" --scope …` | ok — `t-20260910222833-6506`, profile `implementation` |
| overlap | reported by `start`, `majordomus check --overlap` | **four other branches claim paths this task claims**: `readme-integrated-surfaces` (docs, site), `canonical-order` (docs, site, scripts, `.ai/repo/rules/project`, `.ai/repo/ci`), `doctor-passes-from-a-release` (docs, site), `the-subset-is-a-subset` (docs) |
| check | `majordomus check` | ok — 9 findings, 0 failing |
| session | `majordomus session status` | none open in this worktree |
| handover | `majordomus handover --resolve` | none — absence, not a stale match |
| peer board | not announced by this worker | the fleet's row covers it; see §7 for what the board could not have told me |

</div>


Two preflight defects, both real, neither routed around:

**P1 — `MAJORDOMUS_SHARE` breaks the push hook, not only the commit hook.** The variable is
exported into every shell in this repository and points at the *primary* checkout's
`share/`. The commit succeeded under `env -u MAJORDOMUS_SHARE`; the `git push` in the same
script did not carry it and the pre-push hook refused:

```text
majordomus: MAJORDOMUS_SHARE names /Users/korczis/dev/prismatic-majordomus/share, the
distribution of /Users/korczis/dev/prismatic-majordomus — another worktree of this
repository, not this one … Run with: env -u MAJORDOMUS_SHARE
error: failed to push some refs to 'github.com:korczis/prismatic-majordomus.git'
```

The diagnostic is excellent and the remedy is one flag, but the trap is that the variable
is *inherited*, so every hook is a coin flip depending on how the command was invoked. The
fix belongs with the hooks, not with each worker remembering.

**P2 — the audit's own base is already historical.** `origin/master` was `08e4bb252` when
this worktree was created and `06fa25891` by the time the audit finished, roughly forty
minutes later. Any plan this document proposes must assume master moves during the phase
that implements it.

## 2. Current canonical architecture, subject by subject

The full per-concept inventory — canonical owner, storage, schema, discovery, consumers,
mutation path, events, tests, docs and drift — is in
[`DEVELOPMENT_RUNTIME.md`](DEVELOPMENT_RUNTIME.md#the-inventory) and is not repeated here.
This section records only what that inventory does not: the entry and projection subjects
the pack names, and the two-program split that explains most of the rest.

### The split that explains the rest

`bin/majordomus` (shell, `lib/**`) owns the development lifecycle — `start`, `check`,
`finish`, `session`, `context`, `checkpoint`, `handover`, `evidence`, `decision`,
`question`, `plan`, `adr`, `usecase`, `rules`, `doctrine`, `knowledge`, `history`,
`search`. `bin/majordomus-cli` (Rust) owns the capability registry and every projection of
it. Each program rejects the other's commands by name:

```console
$ ./bin/majordomus worktree create feature/development-runtime-model
majordomus: unknown command: worktree
majordomus: worktree is a command of the Rust executable, not of this tool; run: bin/majordomus-cli worktree
```

The diagnostic is precise, which is the point: the split is deliberate and documented, and
it is still the reason the Cockpit cannot reach the lifecycle. A surface served by the Rust
executable can only project what the Rust executable knows.

## 3. Running the system

Started by the repository's supported path, in this worktree, against this worktree's own
build. The primary checkout's server was left alone.

### Startup, reuse and the fleet

```console
$ ./bin/majordomus-cli serve status          # before
standing   absent
desired    127.0.0.1:8741  version 0.5.0
repository 4ca83e493da37139e9b0324377ab2c20  (this is a linked worktree)

$ ./bin/majordomus-cli serve ensure
ready http://127.0.0.1:62354 pid 44871 version 0.5.0 (started by this call)
                                                          0.45s total

$ ./bin/majordomus-cli serve ensure          # again
ready http://127.0.0.1:62354 pid 44871 version 0.5.0
                                                          0.03s total
```

Start is idempotent and reuse costs 30 ms. Two structural observations from the same
output:

- **One server per checkout, not per repository.** `desired` is `127.0.0.1:8741` for every
  checkout, so the first one wins the port and each linked worktree falls back to an
  ephemeral one (this one got 62354). The repository id is shared; the server is not. The
  doc that says "one shared server per repository" is describing the *primary* checkout.
- **The fleet is mostly dead.** `serve status` enumerated 96 checkouts, of which 5 were
  serving and 4 of those 5 were `outdated`. The list includes long-gone scratch trees under
  `/private/tmp/classify-*` and `/private/tmp/force-merge-*`. Nothing reaps the register.

### The stale-server class, measured from both sides

```console
$ ./bin/majordomus-cli serve status
  outdated  /Users/korczis/dev/prismatic-majordomus (master)  http://127.0.0.1:8741  peers 5
            the executable it was started from … has been replaced since
            (it is serving code that is no longer on disk)
  outdated  …/feature/swagger-offline           http://127.0.0.1:60204  peers 0
  outdated  …/feature/the-cockpit-shows-the-board http://127.0.0.1:53226  peers 0
  outdated  …/fix/stale-runtime-is-loud          http://127.0.0.1:8799   peers 0
  outdated  …/fix/worktrees-page-is-fast         http://127.0.0.1:8792   peers 0
            the server published no version, so it is older than this executable (0.5.0)

$ curl -s http://127.0.0.1:8741/api/v1/server        # the outdated server, asked directly
HTTP 200   standing: ready
```

**The staleness is only visible from outside.** The lease reader in a fresh executable can
tell that the running binary was replaced; the running server reports itself `ready` and
its root document carries no HEAD to check. Five peers were attached to that server during
this audit, every one of them reading a projection of code that no longer exists. This is
the single most dangerous class in the audit, because every measurement anyone takes
through that server is quietly wrong — including, as the coordinator of this fleet found,
measurements of capability exposure.

### Surfaces

Every request below was made against `http://127.0.0.1:62354`.

<div class="overflow-x-auto" tabindex="0">

| Request | Status | Type | Size | Time |
|---|---|---|---|---|
| `GET /` | 200 | `application/json` | 2 345 B | 0.6 ms |
| `GET /` with `Accept: text/html` | 200 | `text/html` | — | — |
| `GET /cockpit` | 200 | `text/html` | 25 913 B | 21.6 ms |
| `GET /cockpit/capabilities` | 200 | `text/html` | 59 528 B | 1.8 ms |
| `GET /cockpit/executions` | 200 | `text/html` | 17 599 B | 1.4 ms |
| `GET /cockpit/worktrees` | 200 | `text/html` | 78 098 B | **18 291.9 ms** |
| `GET /cockpit/graphs/composed` | 200 | `text/html` | 1 192 144 B | 39.3 ms |
| `GET /openapi.json` | 200 | `application/json` | 783 220 B | 13.6 ms |
| `GET /swagger` | 200 | `text/html` | 6 382 B | 0.5 ms |
| `GET /docs/` | **503** | `application/json` | 251 B | 0.4 ms |
| `GET /api/v1/capabilities` | 200 | `application/json` | 1 247 611 B | 7.8 ms |
| `GET /api/v1/health` | 200 | `application/json` | 2 737 B | 0.5 ms |
| `GET /api/v1/live`, `/api/v1/ready` | 200 | `application/json` | — | — |
| `GET /api/v1/peers`, `/api/v1/executions`, `/api/v1/plan/status` | 200 | `application/json` | — | — |
| `GET /api/v1/does-not-exist` | 404 | `application/json` | 138 B | 0.4 ms |

</div>


Content negotiation on `/` works. The 404 body is a model of what an error should say:

```json
{"error":{"code":"not_found","message":"no route GET /api/v1/does-not-exist; the routes are listed at /openapi.json"}}
```

Two findings: **`/docs/` is 503 in any checkout that has not built the site**, because the
surface is served from `target/web/docs`, which is a build output. And **the health probes
are at `/api/v1/live` and `/api/v1/ready`**, not under `/api/v1/health/…`; `health.live` and
`health.ready` are the only two built-in capabilities with no MCP exposure, which the
OpenAPI document and the registry agree on.

### The canonical API, measured through its own document

```console
$ python3 -c "…json.load(open('openapi.json'))…"
openapi 3.1.0 | info.version 0.5.0 | paths 78 | operations 78
non-GET operations: [('/api/v1/executions/cancel','post'),
                     ('/api/v1/executions/start','post'),
                     ('/api/v1/peers/announce','post')]
```

78 paths, 78 operations, one operation per path, and **exactly three of them mutate**. The
OpenAPI document is a faithful projection of the registry, and what it faithfully projects
is a runtime with no development mutations in it.

### MCP

```console
$ POST /mcp initialize
protocol 2025-06-18 | server {'name':'majordomus','version':'0.5.0'} | caps ['resources','tools']
Mcp-Session-Id: 44871-1-15feeba0
$ POST /mcp notifications/initialized          → HTTP 202
$ POST /mcp tools/list                          → 76 tools
$ POST /mcp resources/list                      → 1189 resources, no cursor
$ POST /mcp resources/read majordomus://adr/adr-0040
read ok | text/markdown | "--- schema: adr/v1 id: adr-0040 …"
$ POST /mcp tools/call majordomus_plan_status   → isError false, 5 656 chars
$ POST /mcp tools/call majordomus_plan_done     → {"code":-32602,"message":"unknown tool: majordomus_plan_done"}
```

The MCP surface is complete and correct over what the registry holds — including this
branch's own new ADR, served as a resource minutes after it was written. The last line is
the architecture in one error message: there is no tool for a plan transition, because
there is no capability for one.

### Peers

The board is connection-scoped and was proved so by accident: the MCP probe above appeared
as `p1 / phase01-probe / http / attached: true`, and once the probe's connection closed the
count returned to `0`. An announcement belongs to a connection, exactly as `AGENTS.md`
warns; a worker that reconnects keeps its work and loses its place.

### Executions — the disjoint-plane defect

```console
$ curl /api/v1/executions                     → 0 count / 0 remembered
$ ./bin/majordomus-cli run objects.verify &   # while polling the server every second
  t+1s server sees: 0 count, 0 active, 0 remembered
  (run finished at t+1s)
run exit=0                                    → "OK read — 976 current, 0 drifted"
$ curl /api/v1/executions                     → 0 count / 0 remembered
$ curl /api/v1/peers                          → 0
```

An earlier run produced a named, succeeded execution — `x-20260910T222800Z-2adbdce0
health.report` — and the server never saw that one either.

**`majordomus run` does not use the running server.** It builds a registry in its own
process, executes there, streams to its own terminal and exits. The server's execution
store is a different plane in a different process. The consequences for an IDE are exact:
`/cockpit/executions` can only ever show work started *through that server* by
`executions.start`; everything a developer actually runs is invisible to it; and
`docs/EXECUTIONS.md`'s live channel, which works — `GET /events` returns `101 Switching
Protocols` and streams frames — is a window onto a plane almost nothing writes to.

Combined with the non-durability already recorded (`an execution lives in the process that
accepted it`), the execution history an IDE needs does not exist anywhere today.

### Registry, generation and health

```console
$ ./bin/majordomus-cli capabilities validate     → validate: 0 failure(s)
$ ./bin/majordomus-cli generate --check          → generate --check: in sync
$ curl /api/v1/health                            → status "warn", 7 ok / 1 warn
$ ./bin/majordomus doctor        (via the pre-commit hook)
                                                 → doctor: 0 failure(s)
                                                   WARN budget doctor — 51055 ms, budget 3000 ms
                                                   WARN clone unpushed — 48 branches
```

The generation chain is honest: it refused this branch's ADR twice before accepting it,
once for an unsupported YAML construct and once for the wrong heading level, and both
refusals named the file and the reason. `doctor` reports zero failures and takes 51–74
seconds against a 3-second budget — seventeen to twenty-five times over, every commit.

## 4. Gap matrix

*(Pending: the per-capability matrix over the pack's ~100 named capabilities, with one
status each and its evidence.)*

## 5. Drift audit

*(Pending: duplicated semantic definitions by class.)*

## 6. The plan

*(Pending: root causes, dependency graph, phased migration, non-goals, acceptance tests,
risks, files likely to change, generated files that must not be hand-edited, and the
failures that must be fixed before any IDE expansion.)*

## How this was measured

```sh
# the supported start path, reuse, and the fleet
bin/majordomus-cli serve status
bin/majordomus-cli serve ensure

# every surface (B is the address `serve ensure` printed)
curl -s -o /dev/null -w '%{http_code} %{content_type} %{size_download} %{time_total}\n' "$B/<path>"
curl -s -H 'Accept: text/html' "$B/"

# the canonical API document, counted rather than read
python3 -c "import json;d=json.load(open('openapi.json'));print(len(d['paths']))"

# MCP over HTTP, with the session id the initialize response returns
curl -X POST "$B/mcp" -H 'Content-Type: application/json' \
     -H 'Accept: application/json, text/event-stream' \
     -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{…}}'

# the live channel, bounded
curl -s -i -N --http1.1 --max-time 4 -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
     -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' "$B/events"

# whether `run` reaches the server: poll the server while a run is in flight
bin/majordomus-cli run objects.verify & while …; do curl -s "$B/api/v1/executions"; done

# the registry, the generation chain and the lifecycle
bin/majordomus-cli capabilities validate
bin/majordomus-cli generate --check
bin/majordomus start "…" --scope …   ;   bin/majordomus check
```

Counts in this repository go stale. Measure rather than trust a number written here,
including these.
{% endraw %}
