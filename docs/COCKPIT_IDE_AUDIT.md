# The Cockpit as a development surface: baseline, archaeology and gap matrix

A forensic finding, measured on 2026-09-10 in a linked worktree at
`feature/development-runtime-model`, branched from `origin/master` at `08e4bb252`, against
a server this worktree built and started itself. It is phase 01 of the Cockpit IDE /
control-plane pack and it implements nothing: it establishes what exists, proves it by
running it, and says what the smallest safe next phase is.

The architecture it measures against is [ADR 0040](../.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md)
and [`DEVELOPMENT_RUNTIME.md`](DEVELOPMENT_RUNTIME.md): one canonical runtime — the
capability registry of the Rust executable — and every surface a consumer of it. This
document is the evidence behind that decision and the plan that follows from it.

Every claim here was produced by a command, and the command is shown. Where a number
appears without a command, it is a defect in this document.

## The finding in one paragraph

The Cockpit is a faithful projection of a registry that has almost nothing
development-shaped in it. The only capabilities that can mutate anything are
`executions.start`, `executions.cancel` and `peers.announce`, and the OpenAPI document the
server serves has exactly those three non-GET operations to match. Every command of the
development lifecycle lives in a second program, `bin/majordomus`, and every one of its
nodes in the command graph is withheld from every machine surface. So the Cockpit cannot be made an IDE
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

| Step | Command | Result |
|---|---|---|
| contract | read `README.md`, `AGENTS.md`, `CLAUDE.md`, `.ai/README.md` | ok — `AGENTS.md` carries its generated stamp from `.ai/repo/policy.yaml` |
| effective rules | `majordomus rules list --json` | ok — the vendored baseline plus this repository's own resolve in one order; this branch adds one |
| context | `majordomus context` | ok — well inside its line budget, `working_tree clean`, no active task at that moment |
| scoped context | `majordomus context resolve docs/DEVELOPMENT_RUNTIME.md` | ok — resolves to `ai.layer` alone; `docs/` is outside `.ai/` and gets the root chain only |
| worktree topology | `majordomus-cli worktree` | ok — `canonical`, container `…-wt`, trunk `master` |
| collision check | `scripts/collision-check --new <paths>` | ok — nothing else claims `docs/DEVELOPMENT_RUNTIME.md`, `docs/COCKPIT_IDE_AUDIT.md`, the ADR or the rule |
| task lifecycle | `majordomus start "…" --scope …` | ok — `t-20260910222833-6506`, profile `implementation` |
| overlap | reported by `start`, `majordomus check --overlap` | **four other branches claim paths this task claims**: `readme-integrated-surfaces` (docs, site), `canonical-order` (docs, site, scripts, `.ai/repo/rules/project`, `.ai/repo/ci`), `doctor-passes-from-a-release` (docs, site), `the-subset-is-a-subset` (docs) |
| check | `majordomus check` | ok — findings reported, none failing |
| session | `majordomus session status` | none open in this worktree |
| handover | `majordomus handover --resolve` | none — absence, not a stale match |
| peer board | not announced by this worker | the fleet's row covers it; §3 records what the board could not have told me |

Two preflight defects, both real, neither routed around:

**P1 — `MAJORDOMUS_SHARE` breaks the push hook, not only the commit hook.** The variable is
exported into every shell in this repository and points at the *primary* checkout's
`share/`. The commit succeeded under `env -u MAJORDOMUS_SHARE`; the `git push` in the same
script did not carry it and the pre-push hook refused:

```text
majordomus: MAJORDOMUS_SHARE names ~/dev/prismatic-majordomus/share, the
distribution of ~/dev/prismatic-majordomus — another worktree of this
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
- **The fleet is mostly dead.** `serve status` enumerates every checkout it has ever seen.
  Almost all are `absent`, the ones still serving are mostly `outdated`, and the list
  includes long-gone scratch trees under `/private/tmp/classify-*` and
  `/private/tmp/force-merge-*`. Nothing reaps the register.

### The stale-server class, measured from both sides

```console
$ ./bin/majordomus-cli serve status
  outdated  ~/dev/prismatic-majordomus (master)  http://127.0.0.1:8741  peers 5
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
its root document carries no HEAD to check. Peers were attached to that server throughout
this audit, every one of them reading a projection of code that no longer exists. This is
the single most dangerous class in the audit, because every measurement anyone takes
through that server is quietly wrong — including, as the coordinator of this fleet found,
measurements of capability exposure.

### Surfaces

Every request below was made against `http://127.0.0.1:62354`.

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
openapi 3.1.0 | info.version 0.5.0
non-GET operations: [('/api/v1/executions/cancel','post'),
                     ('/api/v1/executions/start','post'),
                     ('/api/v1/peers/announce','post')]
```

One operation per path, and the only ones that mutate are the three the registry declares
as commands. The OpenAPI document is a faithful projection of the registry, and what it
faithfully projects is a runtime with no development mutations in it.

### MCP

```console
$ POST /mcp initialize
protocol 2025-06-18 | server {'name':'majordomus','version':'0.5.0'} | caps ['resources','tools']
Mcp-Session-Id: 44871-1-15feeba0
$ POST /mcp notifications/initialized          → HTTP 202
$ POST /mcp tools/list                          → every tool the registry exposes
$ POST /mcp resources/list                      → every resource, in one page, no cursor
$ POST /mcp resources/read majordomus://adr/adr-0040
read ok | text/markdown | "--- schema: adr/v1 id: adr-0040 …"
$ POST /mcp tools/call majordomus_plan_status   → isError false, a full status document
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

### What the Cockpit may already change

The Cockpit is not read-only, and the shape of its write path is the invariant working
rather than a violation. It carries exactly three actions, and each one is a capability of
kind `command` invoked over its own declared route: **run a capability as an execution**
and **cancel one** (`share/cockpit/executions.js`), and the **generic runner form**, which
is generated from a capability's input schema and can therefore drive any command
capability — today only `peers.announce`, at `/cockpit/capabilities/peers.announce`.

No page carries a hand-written `<form method="post">`; the buttons declare `data-mj-start`,
`data-mj-cancel-route` and `data-mj-effect`, and the browser reads the route from the page
rather than constructing it. `cockpit/mod.rs` refuses every non-GET Cockpit route outright,
and a cross-origin state change is refused and covered by a test.

So the Cockpit's mutation surface is exactly the runtime's mutation surface, which is the
rule this audit's ADR states. The problem is not the Cockpit's write path; it is that the
runtime's mutation surface contains no development semantics for it to reach.

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

One status per capability, from the pack's vocabulary: `canonical+implemented+verified`,
`implemented but duplicated`, `documented only`, `planned`, `partially implemented`,
`broken`, `absent`. "Verified" requires a test that exercises it, named in the row.
"Planned" requires a plan object.

When this matrix was first written, none of it was planned. The search is recorded so the
negative is checkable, and it was re-run against master at `87b9ac659` on 2026-09-11 with
the same result:

```console
$ for kw in palette 'diff view' 'CI status' 'syntax highlight'; do
    printf '%s -> ' "$kw"
    grep -ril "$kw" .ai/repo/project/issues/ .ai/repo/project/milestones/ | wc -l
  done
palette -> 0
diff view -> 0
CI status -> 0
syntax highlight -> 0
```

The only IDE-adjacent plan objects were `I0927` (the Cockpit shows the deployment) and the
`capability-graph` milestone's Cockpit issues, all already shipped.

**That is no longer the state.** The milestone `development-runtime` and the issues `I1501`
through `I1512` were written from this matrix and its §6 plan, and are what the rows below
mean when they say `planned`. §7 records which row each one answers.

### Development

| capability | status | evidence |
|---|---|---|
| capability explorer | canonical+implemented+verified | `capabilities.list/describe/projections`; `/cockpit/capabilities`; `tests/cockpit.rs::a_capability_the_repository_adds_reaches_every_cockpit_surface` |
| command palette | canonical+implemented+verified | `share/cockpit/palette.js`; gate `cockpit-probe` checks the palette family | 
| generic execution runner | canonical+implemented+verified | `pages.rs::runner_form` from the input schema; `tests/cockpit.rs::the_runner_form_is_generated_from_the_input_schema` |
| streaming output | canonical+implemented+verified | `executions.events` + WebSocket; `tests/executions.rs`; `test/cases/100_execution_plane.sh` |
| terminal / REPL | absent | no PTY or shell execution anywhere in `src/`; `palette.js` says in terms "the palette navigates and the terminal executes" |
| source / file navigator | partially implemented | `objects.list`, `directories.list` — the `.ai/**` layer only; no `src/**` tree |
| source viewing | partially implemented | object page renders an indexed file body as text; no code files, no highlighting, no line anchors |
| safe editing workflow | absent | the Cockpit refuses every non-GET route (`cockpit/mod.rs`); no write path, by decision (`COCKPIT.md`) |
| diff viewing | absent | `diff` appears only in changelog commit ranges and worktree fingerprints; no capability, no surface |
| test execution | partially implemented | recipes and cases exist and `commands.list` projects them read-only; runnable only from a terminal |
| formatting / lint / check | partially implemented | offered as text through the workflow catalogue; never executed by the runtime |
| build / release / deploy | partially implemented | `distribution.build`, `deploy.check/list/get` describe it; the doing is shell and CI |
| task / workflow launch | **broken for a surface** | the transitions exist only in `lib/plan.sh` and `lib/start.sh`; unreachable from MCP, HTTP and the Cockpit |
| reusable workflow forms | canonical+implemented+verified | schema-generated controls; the declared benchmark cases load into the runner |
| provider / model interaction | partially implemented | `product.providers` projects provider config; no model client exists in `src/` |

### Planning and SCM

| capability | status | evidence |
|---|---|---|
| repo state | canonical+implemented+verified | `repository.info`; the Cockpit overview; `tests/repository_discovery.rs` |
| branches | canonical+implemented+verified | through `worktree.topology`; `tests/worktree.rs` |
| worktrees | canonical+implemented+verified, **and slow** | `worktree.*`; `/cockpit/worktrees` measured at 18.3 s (§3) |
| commits | partially implemented | conventional-commit parsing and `trace.commit`; no commit browsing surface |
| diffs | absent | — |
| PRs | partially implemented | `trace` derives issue↔PR edges; `scripts/github-sync`; no capability, no surface |
| issues | implemented but duplicated, **and now read canonically** | read `plan.*` and, since `08e4bb252`, `devtask.issue`; write `lib/plan.sh` + `lib/project.awk`; **still no Cockpit page at all** — planned as `I1507` |
| milestones | implemented but duplicated, **and now read canonically** | same split, and `devtask.milestone` since `08e4bb252`; still not in the Cockpit — planned as `I1507` |
| issue↔milestone↔code linkage | canonical+implemented+verified | `trace.issue/commit/report`; `test/cases/98_traceability.sh`; registry and CLI only |
| release / changelog / version | canonical+implemented+verified | `release.changelog`, `release.version`; no Cockpit page |

### AI and runtime collaboration

| capability | status | evidence |
|---|---|---|
| peers | canonical+implemented+verified | `peers.list`, `peers.announce`; `tests/mcp_shared.rs`; **no Cockpit area** — reachable only through the generic runner |
| claims / scopes | implemented but duplicated | Rust `repository.scope*` classifies; `lib/check.sh --overlap` decides containment |
| collisions | partially implemented | `scripts/collision-check` and `check --overlap`; no capability, no surface |
| current task / mandate | partially implemented | `continuity.state` reads it; the write half is `lib/start.sh` only |
| session context | partially implemented | `lib/session_context.sh` only — no Rust owner and no capability |
| handover | partially implemented | `lib/handover.sh`; `continuity.state` surfaces blockers only |
| workflow state | canonical+implemented+verified | the execution state machine; `tests/executions.rs` |
| model / provider surfaces | partially implemented | config projection only |
| executions | canonical+implemented+verified, **and disjoint** | `executions.*`; in-process only, and `majordomus run` never reaches the server (§3) |
| cancellation | canonical+implemented+verified | `executions.cancel`; a non-cancellable capability says so |
| retries | absent — **explicit non-goal** | `execution/mod.rs`: "no durable queue, no retry and no scheduler" |
| live events / logs | canonical+implemented+verified | WebSocket handshake and frames observed in §3; disjoint from the durable ledger |

### Governance

| capability | status | evidence |
|---|---|---|
| effective rules | partially implemented | rules as objects and a rules graph; precedence resolved only in `lib/`; `COCKPIT.md` states "no rule-precedence resolution" as a decision |
| doctrines | canonical+implemented+verified | `lib/doctrine.sh`; `test/cases/17,18`; no project rule is tool-enforced, by doctrine |
| policies | partially implemented | typed and consumed; hand-edited; no capability, no surface |
| ADRs | implemented but duplicated | `lib/adr.sh` and `lib/decision.sh` are two names for one concept, and `release/changelog.rs` still references `.ai/repo/decisions/`; number allocation unguarded |
| context resolution | implemented but duplicated | `directories.list` in Rust and `lib/context.sh` in shell answer one question; the compiled, budgeted context has no capability |
| provenance | canonical+implemented+verified | capability provenance and knowledge-edge provenance, both real |
| governance preflight | partially implemented | `check` and `worktree guard`; the word appears nowhere in `src/`; nothing in the Cockpit |
| violations and remediation | canonical+implemented+verified | `quality.report` findings carry a remedy; `tests/cli_validate_violations.rs` — scoped to Rust API and CLI docs only |
| quality gates | canonical+implemented+verified | `.ai/repo/ci/gates.yaml` and `scripts/ci-plan --check`; **no capability and no view of gate state** |
| "done" obligations | implemented but duplicated | `obligations.closure` reads the contract; `lib/finish.sh` decides it |

### Observability

| capability | status | evidence |
|---|---|---|
| server health | canonical+implemented+verified | `health.report` and its check set; `/cockpit/health`; `tests/health_server.rs` |
| process / runtime state | canonical+implemented+verified | `server.status`; `tests/server_status.rs` — **on no Cockpit page** |
| perf counters | canonical+implemented+verified | `perf.counters`; `/cockpit/activity` — which is not in the sidebar |
| phase timings | canonical+implemented+verified | `tests/hot_path.rs`; a second timing system exists shell-side |
| cache behaviour | canonical+implemented+verified | cache policy and counters; a separate shell cache exists |
| request / execution history | **broken** | in-process only, and the durable half is a different model (`majordomus history`); nothing survives a restart |
| diagnostics | canonical+implemented+verified | the health check set; `doctor` is a different subject by decision |
| CI status | absent | gates and their plan are canonical; nothing reads run state on any surface |
| deploy state | partially implemented | `deploy.*` reads the declared deployment; live state is not there |
| docs / site freshness | partially implemented | a CI answer, not a Cockpit one |
| generated artifact drift | canonical+implemented+verified | `artifacts.list`, `generate --check`, the `generated-registry` health check; two writers of `docs/generated/` remain |

### Docs and knowledge

| capability | status | evidence |
|---|---|---|
| docs | partially implemented | built and mounted at `/docs` — **503 in a checkout that has not built the site** (§3); not a Cockpit area |
| generated reference | canonical+implemented+verified | the generated tree; `tests/generated_documents.rs`; byte-identical twice |
| API reference | canonical+implemented+verified | `/cockpit/api` and the generated OpenAPI |
| Swagger | canonical+implemented+verified | `/swagger` served as infrastructure; not a capability, by declaration |
| knowledge | partially implemented | the compiler is shell-only, no capability |
| ADR graph, rules graph, use-case graph | canonical+implemented+verified | `graph.get`; complete before any JavaScript loads |
| searchable project knowledge | implemented but duplicated | `objects.search` and `/cockpit/search` against `majordomus search` in shell |
| links into GH Pages | absent | the Cockpit never links to the published site |

## 5. Drift audit

The repository reconciles most of its mirrors, and that is the finding as much as the
drift is: the CLI against the registry, every generated artifact against its source, the
plan's two status engines against each other by byte equality, the dispatch table against
the command registry, providers against templates, the version against its two writers.
`docs/HARDCODING_LEDGER.yaml` is the honest register of what remains. What follows is only
what is **not** reconciled, by the classes the pack names.

Two classes came back empty, and the searches are recorded so the negative is checkable:
**handwritten OpenAPI paths — none** (the document is built from the registry; `paths`,
`operationId` and tags have no literal sites), and **workflow arrays — none** (no file
enumerates `just` recipes; the gate `command-graph` refuses a hand-written bridge).

| class | finding | canonical owner | reconciled? |
|---|---|---|---|
| route arrays | `cockpit/nav.rs::areas()` is a literal list, and it is **already behind** the dispatcher: `Executions` and `Quality` are declared in the `Area` enum, rendered by pages and routed, but absent from `areas()` | `cockpit/mod.rs` route dispatch | no — and three surfaces read the stale list: the sidebar, the product model's area validation, and the site's cockpit-areas dataset |
| route arrays | page URLs and API endpoints written as literals throughout `cockpit/pages.rs`, including a hand-written mirror of the navigation on the 404 page | `nav.rs` and the registry's HTTP exposure | no |
| route arrays | `docs/COCKPIT.md`'s route table is missing `commands`, `continuity`, `directories`, `artifacts` and `quality` | `cockpit/mod.rs` | no — the public description of the product is wrong |
| command catalogues | `bin/majordomus`'s `usage()` repeats every command's one-line summary, and `scripts/generate-site-data` publishes *that* copy as site data | `share/commands.yaml` | membership only (`test/cases/30_command_registry.sh`); the summary text is unchecked. This is the open ledger row `command-surface-has-no-owner` |
| duplicated status enums | `share/cockpit/executions.js::isFinalState` decides execution finality in the browser | `execution/model.rs::is_final` | no — a new terminal state means a page that polls forever with every gate green |
| duplicated status enums | `share/cockpit/socket.js` hardcodes the stream control messages although the server publishes the protocol | `executions.protocol` | no |
| duplicated schemas | the generated-artifact contracts under `share/schemas/generated/` are hand-written for artifacts whose shape is a Rust struct | the struct | one direction only: an artifact cannot violate the schema, but the schema can silently lag the type |
| provider arrays | `lib/capture.sh` encodes one provider's settings file, hook names and hook paths as positional strings | `share/providers.yaml`, **which has no field for hooks** | no — the fact has no owner, so `providers-check` cannot see it |
| frontend-only logic | `palette.js` builds its API calls and page URLs from hardcoded paths while every neighbouring module reads them from `data-mj-*` attributes | the page's own data | no — the one component whose purpose is "reach everything" is the one that will miss a moved mount |
| frontend-only logic | `worktrees.js` filters which topology fields "matter" for a reload | a server-side digest | no |
| manual endpoint lists | the loopback address is compiled into every published curl example | the CLI's default port constant | no |
| docs / nav inventories | the site's curated nav plus a literal tile-route list in `site-check` | the generated collections | no — a new collection ships unreachable and unchecked; both are open ledger rows |
| hardcoded counts | **including this branch's own first draft**, which stated a project-rule count that its own new rule invalidated in the same commit | the thing being counted | no decider exists: `project.no-counts-in-prose` says in terms that a reviewer decides it |

The last row is the one worth dwelling on. The rule is blocking and has no enforcement, and
the document that introduced a rule about canonical semantics broke it within one commit.
That is the class in miniature: a mirror is created by someone who knows the rule, while
writing about the rule.

## 6. The plan

### Root causes, ranked

**R1 — the lifecycle and the registry are in two programs, and only one of them has
surfaces.** Everything below follows from this. It is not an accident and it is documented;
it is simply incompatible with the Cockpit becoming a development surface.

**R2 — the runtime has no mutating development capability.** `CapabilityKind::Command` is
used by the execution and peer planes and nowhere else, so there is no typed input schema
for any development transition on any machine surface.

**R3 — the command graph sees the shell tool only at its top level.** Every tool node is a
group, so the exposure policy has nothing to judge and the completion engine nothing to
offer, and the withheld reason for each is a property of the group rather than of the
operation.

**R4 — the lifecycle commands are interactive.** Most tool nodes are withheld because they
ask the person something. Interactivity is not something a surface can work around.

**R5 — there is no development history anywhere.** The durable tier is a local ledger with
its own vocabulary; the live tier is per-process and forgets on exit; and `majordomus run`
writes to neither the server nor, for its own execution, anything durable.

**R6 — the UI-adjacent lists are the ones without keepers.** The repository reconciles its
mirrors well, except at the boundary this pack wants to expand: navigation areas, the
Cockpit's documented route table, and semantics decided in the browser.

### The target flow for one development mutation

```text
Cockpit form / MCP tool / HTTP POST / CLI
        │  typed input, validated against the capability's own schema
        ▼
capability!  kind: command    ← the only place the transition is decided
        │
        ├─▶ the object in its canonical storage   (.ai/repo/project/issues/<id>.yaml)
        ├─▶ a ledger event whose name share/events.yaml declares   (durable record)
        └─▶ the execution stream                                   (live view)
```

Nothing in that diagram is new infrastructure. Every box exists; the arrows from the first
box to the rest are what is missing.

### Dependency graph

```text
A. one mutating development capability, end to end
        │
        ├──▶ B. events: a mutating capability appends to the ledger
        │            │
        │            └──▶ E. Cockpit development pages (issues, sessions, peers)
        ├──▶ C. the lifecycle capabilities (task, session, checkpoint, handover, finish)
        └──▶ D. compiled context as a capability
F. navigation and route derivation ──▶ E     (independent of A–D; do it any time)
```

### Phases

**Phase A — one vertical slice, and nothing else.** Take the plan transitions
(`start`/`verify`/`done`) and declare them as one `capability!` of kind `command` in the
`plan` module, with a typed input, benchmark cases and a registered ledger event. Point
`lib/plan.sh` at it, the way `lib/context.sh` and `lib/capture.sh` already call the Rust
binary. Remove `backing:plan` from `.ai/repo/development-semantics-baseline.txt`.

*Acceptance:* the MCP tool exists and `tools/call` moves an issue; `POST /api/v1/plan/…`
appears in the generated OpenAPI without anyone editing it; the Cockpit's existing generic
runner can drive it with no Cockpit edit; `lib/plan.sh` no longer writes the YAML itself;
`test/cases/99_plan_capabilities.sh` still holds the two engines to byte equality; the gate
`development-semantics` reports one fewer entry and refuses the baseline's return.

This phase is the whole architecture in miniature. If it lands cleanly, every later phase
is the same shape; if it does not, nothing later would have worked either.

**Phase B — one event model.** A capability of kind `command` appends a ledger event whose
name `share/events.yaml` declares, and publishes the same change to the execution stream.
Give the ledger a read capability so the Cockpit can show history at all.

**Phase C — the lifecycle.** `task` start/checkpoint/finish, `session` open/close,
`handover`, `evidence`, in the pattern Phase A established, each one non-interactive so the
exposure policy stops withholding it.

**Phase D — compiled context as a capability.** Owned by the parallel context-compiler
work, not by this phase.

**Phase E — Cockpit development pages.** Issues, milestones, sessions, peers, task. Only
after A–C, and only as projections: a page that computes a status is the defect this whole
document exists to prevent.

**Phase F — derive the navigation.** Reconcile `areas()` against the `Area` enum and the
dispatcher, and derive `docs/COCKPIT.md`'s route table or check it. Independent of the
rest; cheap; prevents the next unlinked page.

### Non-goals

A terminal or PTY on any surface. Editing repository source in the browser. Retries and a
scheduler — the execution plane states in its own module documentation that it has none, by
decision. A second datastore. A second project model. A wholesale port of `lib/*.sh` before
anything else may start. A new noun for anything the repository already names.

### Risks

| risk | mitigation |
|---|---|
| master moves during the phase | it moved during this audit; rebase per landing, and prefer small branches |
| parallel workers own overlapping paths | four branches already claim paths this task claims; `check --overlap` names them, and the peer board does not |
| a second name for an existing concept lands in parallel | one has: a `devtask` module, which documents itself as a join over `plan` and `trace` rather than a second model and holds an equality assertion against `plan.issues`. It is additive and should stay, but its readiness vocabulary is a second status vocabulary derived from the canonical one — Phase A should converge the naming rather than grow a third |
| measurements taken through a stale server | four of the five servers running during this audit were outdated; always measure from a freshly built executable |
| a loaded machine turns slowness into false failures | observed at load 109 on 18 cores; do not read a timing failure as a defect without re-running quiet |

### Files likely to change

`apps/majordomus-cli/src/capability/builtin/plan.rs`, `src/plan.rs`, `share/events.yaml`,
`lib/plan.sh`, `.ai/repo/development-semantics-baseline.txt`, and later
`src/capability/builtin/{continuity,obligations}.rs`, `lib/{session,checkpoint,handover,
evidence,finish}.sh`, `src/cockpit/{nav,pages}.rs`, `docs/COCKPIT.md`.

### Generated files that must never be hand-edited

`docs/generated/**`, `site/data/**`, `site/content/**`, `share/allow/**`,
`share/sections/**`, `AGENTS.md`, `CLAUDE.md`, `.bb/AGENTS.md`, `share/design/*.css`,
`apps/majordomus-cli/src/web/tokens.css`, `apps/majordomus-cli/src/design/tokens.yaml`,
`deploy/Dockerfile`, `fly.toml`, `docs/SITE_CLAIMS.md`, `docs/PLAN_STATUS.md`,
`docs/PAGES_STATUS.md`. Each is written by `majordomus generate` or `scripts/derive` and
checked against a fresh generation; an edit is a reported failure, not a silent divergence.

### Failures to fix before any IDE expansion

| # | failure | why it blocks |
|---|---|---|
| F1 | a server whose executable was replaced reports itself `ready`; the staleness is visible only to an outside reader | every measurement taken through it is quietly wrong, including measurements of what the IDE lacks |
| F2 | `majordomus run` never reaches the running server | there is no execution history for any surface to show, and the one page that shows executions shows a plane nobody writes to |
| F3 | `/cockpit/worktrees` at 18.3 s and `/cockpit/quality` at 4.6 s against 1–4 ms elsewhere | an IDE is used continuously; these are already the slowest thing a person meets |
| F4 | `/docs/` is 503 until the site is built | a developer entering a fresh worktree meets a broken documentation surface |
| F5 | routed Cockpit pages absent from the navigation | new pages are invisible by default, and the probe crawls out of the navigation, so nothing catches it |
| F6 | `doctor` runs 51–74 s against a 3 s budget on every commit | the lifecycle's own cost; it will be paid on every IDE-driven action |
| F7 | `MAJORDOMUS_SHARE` is inherited into hooks and breaks them depending on invocation | it broke this audit's own push |

### The smallest safe next phase

Phase A, and only the `done` transition of it: one `capability!` of kind `command` that
moves one issue, with its ledger event and its use case. It touches one module, one shell
file and one baseline line; it is provable end to end through four surfaces in a single
sitting; and it either establishes the pattern for everything else or shows immediately
that the pattern is wrong.

## 7. Re-verification on 2026-09-11, and the plan objects

The audit above was measured against `08e4bb252`. This section re-measures its load-bearing
claims against master at `87b9ac659`, from an executable built in the worktree that took the
measurements rather than through any running server — §3's own warning about stale servers
applies to this section as much as to the original. A row is corrected where a command says
it has changed, and marked unverified where no command was run.

`P2` of §1 predicted this: the audit's base was historical before the audit finished. It was
`08e4bb252` at the start, `06fa25891` at the end, and `87b9ac659` a day later.

### The central finding still holds

```console
$ python3 -c "import json;c=json.load(open('docs/generated/registry.json'))['capabilities'];\
              print([x['id'] for x in c if x['kind']=='command'])"
['executions.cancel', 'executions.start', 'peers.announce']

$ python3 -c "import json;d=json.load(open('docs/generated/openapi.json'));\
              print(sorted((p,m) for p,o in d['paths'].items() for m in o if m!='get'))"
[('/api/v1/executions/cancel', 'post'), ('/api/v1/executions/start', 'post'),
 ('/api/v1/peers/announce', 'post')]
```

Three mutating capabilities, three non-GET operations, none of them development-shaped.
The paragraph this document opens with is unchanged after a day of landings.

### What did change: two capabilities and one module

```console
$ git diff --stat 08e4bb252 87b9ac659 -- docs/generated/registry.json   # then compared as sets
capabilities added:   devtask.issue, devtask.milestone
capabilities removed: none
modules added:        devtask
counts:               78 → 80
```

Both are `kind: query` — they read and write nothing — but they are the runtime's first
canonical answer to *an issue as work*: readiness, waves, blockers, the parallel subsets,
each field carrying whether a person authored it or a machine derived it, as a pure function
of the canonical records and explicitly so that a surface reading it derives nothing itself.
Two rows of §4 are corrected above because of it. The `devtask` module's own documentation
states it is not a second project model, and §6's risk table already names the convergence
this creates; that risk is now carried by the milestone rather than only by this document.

### The ADR: no renumbering was needed

ADR 0040 is on master and is this document's decision.

```console
$ git log --oneline -1 87b9ac659 -- .ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md
93c5f3f87 docs(architecture): name the canonical development runtime and make it enforceable

$ git merge-base --is-ancestor feature/development-runtime-model origin/master && echo merged
merged
```

The whole of `feature/development-runtime-model` — this audit, `DEVELOPMENT_RUNTIME.md` and
the decision — landed on master between the audit finishing and this re-verification. No
number had to be found and none was allocated.

### Two further Cockpit findings, measured here

**`/cockpit/graphs/composed` is a megabyte in one document.** Measured against a server this
worktree built and started:

```console
$ curl -s "$B/cockpit/graphs/composed" -o composed.html -w '%{http_code} %{size_download} %{time_total}\n'
200 1196619 0.059885
$ wc -m < composed.html ;  grep -o '<tr' composed.html | wc -l ;  grep -c 'page=' composed.html
1196549
4729
0
```

This follows a contract rather than contradicting one: `docs/COCKPIT.md` states that a
detail page pages nothing, and gives the reason — the drawing is an enhancement, a reader
without JavaScript has only those lists, and a reader checking whether a path is in the
manifest must find it with the browser's own search. The contract is right; at 4,729 rows
its cost has outgrown it, because a megabyte is not searchable by a person either. A second
finding came with it: the sweep in `apps/majordomus-cli/tests/cockpit.rs` names
`/cockpit/graphs`, `/cockpit/graphs/topology` and `/cockpit/graphs/registry`, and not
`/cockpit/graphs/composed` — the largest page the Cockpit serves is one no test renders.
Planned as `I1509`.

**`/cockpit/search` does *not* break the no-JavaScript claim.** This was reported as an
empty shell of 157 characters contradicting `With JavaScript off every page still shows
everything it knows`. It does not reproduce, and the correction matters more than the
claim would have:

```console
$ curl -s "$B/cockpit/search"        | wc -m        # no query
16212
$ curl -s "$B/cockpit/search?q=scope" | wc -m       # a query, server-rendered
42865                                               # 164 result rows, no script involved
```

The 157 characters are the *visible text of the `<main>` region with no query given* — 160
by my count — and that text is `Type something. The search is the repository's own:
case-insensitive, over identities, titles, descriptions and content.` The page carries a
real `<form method="get" action="/cockpit/search">`; submitting it without JavaScript
returns the results in the HTML. A page with no query knows nothing to show, so the claim
holds. No issue was written for it. It is recorded because the measurement that produced
the number was right and the conclusion drawn from it was not, which is the more expensive
of the two mistakes.

### The index was degraded, and the cause was not in the repository

```console
$ curl -s http://127.0.0.1:8741/api/v1/repository | grep -o '"state":"[a-z]*"'
"state":"degraded"
```

Four files claimed `majordomus://session/s-20260909152316-024f`, so every claimant was
excluded and the session was absent from the registry that exists to hold it. One was
committed; three were written the next morning, thirteen and fifteen seconds apart, by a
closer re-closing a session closed the evening before, and all three sat *staged and
uncommitted* in the primary checkout — which is what the running server reads, and why no
committed tree was ever wrong. The commit sets were compared rather than assumed: each of
the three contains all 94 commits of the committed record and 202 more. The complete record
is now the one in the tree and the partial one is gone; the redundant files were unstaged
and left on disk. The writer is unchanged and is `I1503`.

### Rows not re-verified

Every row of §4 not named above is carried through unchanged and **unverified against
`87b9ac659`**. The capability set moved by exactly two between the two commits, so a row
whose evidence is a capability or a test that still exists is unlikely to have changed —
but "unlikely" is not a measurement, and this document's own rule is that a number without
a command beside it is a defect in it.

Three timings in §3 are explicitly *not* re-measured and should not be quoted: the 18.3 s
for `/cockpit/worktrees`, the 4.6 s for `/cockpit/quality`, and the 51–74 s for `doctor`.
The first is being fixed on `fix/cockpit-answers-quickly` and this document does not
duplicate that work. `doctor` was observed at 83.6 s during this session's own pre-commit
hook, on a machine running many suites at once, which §6's risk table says is exactly the
condition under which a timing must not be read as a property of the command.

### What §6's plan became

| plan item | object |
|---|---|
| F2 — `run` never reaches the server | `I1501` |
| F1 — a stale server reports itself ready | `I1502` |
| the duplicate session identity | `I1503` |
| Phase A — one mutating development capability | `I1504` |
| Phase B — one event model | `I1505` |
| Phase C — the lifecycle | `I1506` |
| Phase E — Cockpit development pages | `I1507` |
| Phase F / F5 — navigation derived from the dispatcher | `I1508` |
| the composed graph | `I1509` |
| this matrix is checked against the plan | `I1510` |
| F4 — `/docs/` is 503 in an unbuilt checkout | `I1511` |
| CI status has no reader | `I1512` |
| F3 — `/cockpit/worktrees` is slow | not written: `fix/cockpit-answers-quickly` |
| Phase D — compiled context | not written: owned by the context-compiler work |
| F6 — `doctor` over budget | not written: reported by `doctor` itself on every run |
| F7 — `MAJORDOMUS_SHARE` inherited into hooks | not written: it broke this session's commit too |

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
