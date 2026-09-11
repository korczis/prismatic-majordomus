+++
title = "The Cockpit"
description = "the Cockpit: the registry rendered as pages for a person, what makes it a projection rather than a dashboard, the graph and health models, the browser layer and what happens without it, the security decisions, the asset pipeline"
weight = 41
[extra]
source = "docs/COCKPIT.md"
+++

{% raw %}

`majordomus serve` and `majordomus mcp` bind one shared server per repository. That server
already answers machines: MCP tools and resources, HTTP routes under `/api/v1/`, an OpenAPI
document and a Swagger UI over it. The Cockpit is the same server answering a person, at
`/cockpit`.

It is a **projection**, in this repository's sense of the word: one more thing derived from
the canonical capability declarations and the index, holding no model, no catalogue and no
verdict of its own. The decision is
[ADR 12](../.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md); the rules
it is bound by are `project.interfaces-are-projections`,
`project.rust-canonical-declaration`, `project.rust-benchmark-coverage` and
`project.rust-hot-path`. Behaviour as implemented and tested; where this document and the
executable disagree, the document is wrong and changes in the same commit.

```text
ONE CANONICAL DECLARATION   capability! { id, title, description, input, output,
                                          stability, exposure, tags, cache?, handler }
        ↓
CAPABILITY REGISTRY         + every declarative object of the layer
        ↓
DERIVED PROJECTIONS         MCP · HTTP · OpenAPI → Swagger UI · CLI · benchmark targets
                            · docs/generated/* · the website's /registry/ pages
                            · THE COCKPIT — pages, navigation, search, palette, runner, graphs
```

## Getting there

```sh
majordomus serve                 # or `majordomus mcp`, which binds the same shared server
```

The server logs the URL it bound. Open that URL: a browser is redirected to `/cockpit`, and
a client that does not ask for HTML gets the JSON index it has always got. Both are on the
same port; there is no second process and no second command.

Nothing needs building for it to work. If `share/cockpit/` has no compiled stylesheet the
pages still render, say so, and remain fully usable.

## What is on it

<div class="overflow-x-auto" tabindex="0">

| Route | What it shows | Derived from |
|---|---|---|
| `/cockpit` | repository identity, git state, index state, the registry counted, every diagnostic, the health summary | `repository.info`, `health.report` |
| `/cockpit/capabilities` | every capability, filtered by module, kind, source or text | the registry |
| `/cockpit/capabilities/<id>` | one descriptor in full: schemas, projections, cache and benchmark policy, provenance, examples, and a form that runs it | the descriptor and its `BenchmarkCases` |
| `/cockpit/executions` | what this process has run and is running, with the counts beside it; follows the live channel and updates itself | `executions.list` |
| `/cockpit/executions/<id>` | one execution: its state, steps, progress, diagnostics, live output, output or error, and the input as it was stored; a stable URL a reload restores from | `executions.get`, `executions.events` |
| `/cockpit/objects` | the declarative objects of the layer, by kind | `objects.list` |
| `/cockpit/object?uri=` | one object: front matter, provenance, content as it is | `objects.get` |
| `/cockpit/graphs` | every graph this executable derives | `graph.list` |
| `/cockpit/graphs/<id>` | one graph: the drawing, the vocabularies, and every node and edge as tables | `graph.get` |
| `/cockpit/graphs/topology` | the registry graph in three dimensions — optional | `graph.get` (`registry`) |
| `/cockpit/health` | one check per dimension, each with the engine that decided it and the command that reproduces it | `health.report` |
| `/cockpit/api` | every HTTP route the registry projects, and the projection's own | the registry |
| `/cockpit/search` | capabilities and objects matching one query | the registry, `objects.search` |
| `/cockpit/worktrees` | the branch-to-worktree topology: container, trunk, every worktree with its standing, uncommitted work and diagnostics, every branch without a worktree, the migration plan with the command that applies it; reloads itself when the topology changes | `worktree.topology`, `worktree.migration_plan` |
| `/cockpit/activity` | this process's counters and phases, and a running plot of them | `perf.counters` |
| `/cockpit/assets/<file>` | the stylesheet, the scripts, the vendored libraries | `share/cockpit/` |

</div>


Every route is a deep link: the filters are query parameters, a refresh loses nothing, and
a page can be sent to somebody.

## How a listing of the whole layer is read

The two listings that hold everything — nine hundred capabilities, nine hundred objects —
are read a page at a time and entered by their parts:

- **A page is fifty rows.** `?page=` says which; a number past the end is the last page
  and never an error. The control under the table says which rows are being shown, and
  offers the first page, the last, the neighbours of this one, and a gap for the rest.
- **Above the table are the listing's own parts** — the registry's modules on
  `/cockpit/capabilities`, the index's kinds on `/cockpit/objects` — each with how many
  it holds *under the filters in force*. They are the same catalogues the sidebar shows,
  put where the listing is: a set of nine hundred rows is entered by its module or its
  kind rather than scrolled.
- **A link never loses a filter and never keeps a page number.** Paging carries every
  filter with it; picking a part starts that part at its first page.

A **detail** page pages nothing. A graph's nodes and edges, and the artifact manifest, are
listed whole: the drawing is an enhancement over those lists, a reader without JavaScript
has only them, and a reader checking whether a path is in the manifest must be able to
find it with the browser's own search.

## What makes it a projection and not a dashboard

**Every page asks a capability.** A page calls `Context::execute`, which is the call MCP and
the HTTP routes make. It is counted in the same perf counters, answered from the same cache
and bound by the same validation. `tests/cockpit.rs` asserts that serving every page moves
none of the counters that must only move at startup.

**Nothing in it is a list.** The sidebar's catalogues are the registry's modules, the
index's kinds and the graph derivation table. The capability explorer is the registry
filtered. The palette reads `/api/v1/capabilities`, `/api/v1/graphs` and `/api/v1/objects`,
and takes the Cockpit's own pages out of the navigation the server already rendered.

**The runner is generic.** A capability's form is generated from its input schema: the
control follows the JSON type (string, integer, number, boolean, enumeration, or a JSON
textarea for anything structured), the description is the property's own doc comment, and
requiredness is the schema's. Submitting sends the request the capability's own binding
expects — a query string for a `GET`, a JSON body for a `POST` — to the capability's real
route. There is no runner endpoint, no proxy and no second validation.

**The examples are the benchmark cases.** The inputs shown on a capability's page and the
"load into the runner" buttons are `BenchmarkCases`, the same inputs the benchmark runs and
the OpenAPI document shows.

The behavioural claim is one test:
`tests/cockpit.rs::a_capability_the_repository_adds_reaches_every_cockpit_surface` writes a
rule into a disposable repository and asserts it reaches the capability listing, a page of
its own, the object explorer, the search and the listing the palette reads — with no line
of the Cockpit written for it.

## Graphs

Graphs are canonical in Rust (`src/graph.rs`): a `Graph` is an id, its words, a node-kind
and an edge-kind vocabulary that say what each shape means, sorted nodes and edges, and the
metadata a reader needs before drawing — counts, whether the result is acyclic, and whether
the derivation stopped at its node limit.

<div class="overflow-x-auto" tabindex="0">

| Graph | What it shows | Derived from |
|---|---|---|
| `registry` | modules, the capabilities they compose, the file each was declared in, and the MCP / HTTP / CLI projections | the registry |
| `layer` | every directory an object was discovered in, nested, with the kinds each holds | object provenance |
| `rules` | every rule and the rules it declares it depends on | `depends_on` front matter |
| `adrs` | every decision, what it supersedes, and the rules, claims, files and tests it put in force | ADR front matter |
| `use-cases` | every use case and the commands, doctrines and claims it names | use-case front matter |

</div>


Adding one is a function and one line in `graph::DERIVATIONS`. It then appears in
`graph.list`, at `/api/v1/graph`, as an MCP tool answer, in the Cockpit's navigation, on a
page of its own, and as a benchmark case — because `GraphInput::benchmark_cases` is
`graph::ids()`.

Invariants the property and unit suites hold: node ids are unique, no edge references a
node the graph does not hold, and the same tree and executable produce the same bytes (no
clock, no absolute path, no discovery order).

A drawing library is a consumer. The translation to Cytoscape's element shape is one
function in `share/cockpit/graph.js`; a projection to Mermaid, Graphviz or a terminal would
rebuild no semantics.

## Health

`health.report` reports one check per dimension. Every check carries **who decided it** and
**what reproduces it**, because a health report that decides things itself is a fourth
opinion:

<div class="overflow-x-auto" tabindex="0">

| Check | Decided by |
|---|---|
| the layer as it was read | the index's own diagnostics |
| the capability registry | the registry builder, which refuses to build on a duplicate or a malformed exposure |
| the declared scope | the scope declaration the index was built under |
| version control | git, asked once at startup |
| benchmark coverage | the benchmark projection's coverage — the same one `bench coverage --check` reads |
| committed projections | the same rendering `majordomus generate` writes, compared with what is committed |
| the shared server | the decision `server.status` makes, from this checkout's lease and one probe of the server it names |
| attached clients | the in-memory peer board of this process |

</div>


The server check reports where this checkout's shared server stands — `absent`,
`starting`, `ready`, `outdated` or `stale` — with its address, the version it serves and
the reason when the answer is not `ready`. `absent` and `starting` are `ok`: a checkout
nobody serves is not an unhealthy one, and a lease still binding resolves itself within the
bind grace. `outdated` and `stale` are `warn` with the reason as a finding: a server
answering from code this tree no longer has, and a lease naming an address nobody answers
at, are both somebody's to clear and neither stops this process from serving. It is the
fourth of this crate's four readings of "ready", and the only one that belongs in a health
report; `docs/MCP.md` names all four and what each is for.

It deliberately does **not** re-render every committed projection on each call: that would
rebuild canonical state per request. It compares the one artifact derived from code alone
(`docs/generated/registry.json`) and names `majordomus generate --check` as the complete
answer.

It is also not `majordomus doctor`. The shell tool's doctor decides whether Majordomus is
*wired into this repository*; these checks decide whether what this process *serves* is
sound. Different subjects, and the Rust server dispatches no shell.

## The browser layer

Progressive enhancement in the strict sense. Every page is complete HTML before any script
runs; the graph pages carry every node and edge as tables, and the optional views say where
the same facts are as text.

<div class="overflow-x-auto" tabindex="0">

| File | What it adds | Needs |
|---|---|---|
| `cockpit.js` | the one Alpine component (theme, palette state), the shared helpers | `vendor/alpine.csp.min.js` |
| `palette.js` | the command palette (`Ctrl`/`Cmd` + `K`), entries from the registry | — |
| `runner.js` | the generic capability runner | — |
| `graph.js` | the Cytoscape view: pan, zoom, fit, search, neighbourhood focus, a details drawer | `vendor/cytoscape.min.js` |
| `topology.js` | the registry graph in three dimensions | `vendor/three.module.min.js` |
| `activity.js` | a running plot of the execution and cache counters | `vendor/p5.min.js` |

</div>


If a vendored library is absent the frame says so and the page keeps working. None of them
is on the critical path, each is fetched only by the page that uses it, and the two visual
ones respect `prefers-reduced-motion`, pause when the tab is hidden and dispose what they
allocate.

Why Three.js is there at all: the registry graph is layered — modules compose capabilities,
capabilities are declared in files and project onto three interfaces — and a flat drawing
must choose between showing the layers and showing the fan-out. One plane per layer shows
both. Why p5 is there: the counters are already a table, and a table cannot show whether
the executions arriving now are being answered from the cache. Both are optional views of
data the page also prints.

## Security

- **Escaping by construction.** Markup is a tree of `El` values; the only way text reaches
  the output is a method that escapes it. There is no template string, and the tests render
  a rule whose title is a script tag.
- **A strict content-security policy.** `default-src 'none'; script-src 'self' 'sha256-…'`
  — no `unsafe-inline`, no `unsafe-eval`, no remote origin, `frame-ancestors 'none'`. The
  one inline script is the pre-paint theme bootstrap, allowed by the digest of its own
  bytes, computed from the constant itself so the two cannot drift. This is why Alpine's
  **CSP build** is what is vendored and why every `x-` attribute names a property or a
  method rather than an expression.
- **Browser mutation protection.** A request that carries an `Origin` header came from a
  page in a browser. For anything but `GET` and `HEAD`, that origin must be this server's
  own; otherwise it is refused with 403 before a handler runs. A client that is not a
  browser sends no `Origin` and is unaffected. Reads are already contained: this server
  sends no CORS headers, so a cross-origin page cannot read an answer. There is no session
  and no cookie, so there is no token to add — a token bound to nothing defends nothing.
- **No state change through `GET`.** The Cockpit's own routes answer `GET` only; a command
  is `POST` to its own route, and the runner marks it as one.
- **Path handling.** An asset name is refused before the filesystem is touched if it holds
  anything but `[A-Za-z0-9._-/]`, an empty segment, `.` or `..`; the resolved path is then
  checked to be inside the asset directory, which closes the door a symlink would open.
  Only a closed set of text media types is served.
- **Loopback by default.** `serve` binds `127.0.0.1`; binding anything else logs what it
  means.
- **Nothing writes.** Every capability the Cockpit can reach is a query, or the one command
  that changes this process's own memory. Nothing in it writes to the repository.

## Assets

`share/cockpit/` is where this tool keeps the Cockpit's files, the way `share/kinds.yaml`
and `share/schemas/` are where it keeps its kinds. They are read from disk, once, and
served from memory with the digest of their bytes in the URL — so a changed file is a
changed URL, and a versioned asset is answered `immutable` for a year.

```sh
scripts/cockpit-assets            # compile the stylesheet, vendor the pinned libraries
scripts/cockpit-assets --check    # fail if the committed stylesheet differs from its source
```

<div class="overflow-x-auto" tabindex="0">

| File | Committed? | Why |
|---|---|---|
| `cockpit.css` | yes | a generated artifact, compiled from `src/cockpit.css` with Tailwind and Flowbite's theme over the shared design sheets; drift-checked |
| `src/cockpit.css` | yes | the source: what only the Cockpit has (shell, palette, runner, log, canvases), one semantic class per component, over the tokens and primitives it imports from `share/design/` |
| `favicon.svg` | yes | a generated copy of the canonical mark `share/design/brand/logo-mark.svg`, written by `majordomus generate design` |
| `*.js` | yes | written by hand, no build step |
| `vendor/alpine.csp.min.js` | yes | the interaction depends on it and it is 70 kB |
| `vendor/{cytoscape,three.module,three.core,p5}.min.js` | no | ~2 MB, lazy, optional, every page complete without them |

</div>


There are no utility classes in the Rust that renders the pages. Everything the markup
names is defined in `src/cockpit.css` or in the shared `share/design/primitives.css` as a
composition of Tailwind utilities over the design tokens, which keeps the design readable
and makes the compiled stylesheet a function of those files and the pinned versions alone —
which is what makes `--check` exact. No colour, size, status colour, theme key or mark is
decided in the Cockpit: every one comes from `share/design/tokens.yaml` through the
generated sheets, the same sheets the published site imports, and `scripts/ci/design-check`
refuses a second decision. The Design page (`/cockpit/design`) renders the declaration the
executable was built with and says whether the stylesheet it loaded agrees.
See [`DESIGN_SYSTEM.md`](@/docs/design-system.md).

Node is a **build** dependency. The Rust executable serves these files from disk and has no
idea they were built.

## Performance

The pages are benchmark targets, declared beside `GET /` and `GET /docs`, so
`bench coverage --check` counts them and a baseline can regress on them:
`system.http.cockpit_overview`, `system.http.cockpit_capabilities`,
`system.http.cockpit_graph`.

What the numbers are on any given machine is measured, never written here — the evidence
lives under `.ai/repo/benchmarks/rust/` and `docs/generated/benchmarks.md`. What is
guaranteed is structural: no page rebuilds canonical state, an asset is read from disk once
per process, a graph is cached by the executor, and no library that a page does not use is
fetched.

## Adding to it

**A capability.** Write the `capability!` block and run `majordomus generate`. It appears in
the Cockpit's listing, on a page of its own with a generated form, in the search, in the
palette, in the registry graph and as a benchmark target. Nothing in `src/cockpit/` is
edited.

**A graph.** Write the derivation and add one line to `graph::DERIVATIONS`. It appears in
`graph.list`, at `/api/v1/graph`, in the navigation, on a page, and as a benchmark case.

**A page.** That is the one thing that is a Cockpit edit, because a page is the Cockpit's
own shape: a function in `pages.rs` returning a `Page`, and one arm in `Cockpit::route`. A
page that needs a script names it with `.script("name.js")`, which is what keeps a graph
library off the landing page.

**A component.** One class in `share/cockpit/src/cockpit.css` and one function in `view.rs`.
Never a utility class in the Rust.

## Testing

`apps/majordomus-cli/tests/cockpit.rs` runs a real server over a real socket against a
disposable repository:

- every page renders complete HTML with the shell and the security headers,
- a capability added to the repository reaches every Cockpit surface,
- the runner form is generated from the input schema,
- a graph page lists every node and edge before any library loads,
- the health page shows the verdicts the engines reach,
- assets are immutable by digest and no traversal escapes the directory,
- a state-changing request from another origin is refused and a read is not,
- the index answers JSON to a client and points a browser at the Cockpit,
- repository content reaches the page as text and never as markup,
- serving every page rebuilds nothing canonical.

The unit tests in `src/cockpit/` cover the escaping (both contexts), the void elements, the
status-word-to-class mapping, the asset cache and the path refusals, the CSP digest, and
the navigation being the registry's rather than a list.

## What it deliberately does not do

Four things a control plane is expected to have, left out on purpose. Each is a decision
rather than a gap, and each names what would have to change first.

**No streaming for its own sake.** This section used to say there was no live transport at
all, and gave the reason: a stream to make a counter update without a poll is a noun added
to the core for a convenience, which `optional-complexity` exists to refuse. It also said
what would change the trade — "when something genuinely long-running arrives, the stream
will be worth its own decision, and the typed event envelope belongs in that decision rather
than ahead of it".

That arrived, and that is [ADR 22](../.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md):
a capability that reads every file of the layer takes long enough to watch rather than wait
for, so an **execution** has an identity, typed events and a live channel at `GET /events`
([`EXECUTIONS.md`](@/docs/executions.md)). What did *not* change is the trade the paragraph
protected. The activity page still polls, because counters are a convenience; no capability
became a stream, because the fourth kind was drafted and dropped; and the live channel
carries execution events and nothing else. A page that wants a number to tick has the same
answer it had before.

**No rule-precedence resolution.** "What instructions apply at this path?" is a question the
Cockpit is the right place to answer and the wrong place to *decide*. The shell tool owns
context resolution (`majordomus context resolve`), and a second implementation in Rust would
be a second opinion about precedence — exactly the drift every rule here is written against.
What the Cockpit does instead is show the inputs: every rule as an object, the rule
dependency graph, and the directory contracts as objects of kind `context`. The resolution
itself waits for one engine both tools can call.

**The health page is not `majordomus doctor`.** Doctor decides whether Majordomus is *wired
into this repository* — the hooks, the projections, the retention, the state. `health.report`
decides whether what this process *serves* is sound. Different subjects with different
engines, and the Rust server dispatches no shell, so there is no third thing that runs both.
A reader who wants both runs both; each names the other's territory.

**No write path.** Every capability the Cockpit can reach is a query, or a command that
changes this process's own memory — starting an execution and cancelling one are two of
those. Nothing in it writes to the repository, and a capability that did would need its own
decision (ADR 12 says so explicitly) and would say so in its own execution policy, which is
what the confirmation on the Run button reads.

## In a browser

`scripts/cockpit-probe` measures the Cockpit against a running server. Nothing in it lists
a route: the areas are crawled out of the shell's own navigation — which the server rendered
from the registry, the index and the graph derivations — and then one route per capability,
per graph and per object comes from `/api/v1/capabilities`, `/api/v1/graphs` and
`/api/v1/objects`. A capability, a kind or a graph added to the backend is probed the first
time it exists.

```sh
just cockpit-probe                      # both sweeps
just cockpit-probe --quick              # the status sweep in full, one of each family in the browser
just cockpit-probe --status             # the status sweep alone; no browser needed
COCKPIT_PROBE_URL=http://127.0.0.1:8741 scripts/cockpit-probe    # a server that is already running
```

Two sweeps, because they answer different questions at different prices:

<div class="overflow-x-auto" tabindex="0">

| Sweep | Over | Asserts |
|---|---|---|
| status | **every** derived route | 200, `text/html`, and the security headers on the shell |
| browser | every area plus a sample of each generated family, at 390, 1024 and 1600 px | the shell, the stylesheet applied, no horizontal overflow, no console error, no failed request |

</div>


The browser sweep is the part a status code cannot reach:

- **Alpine** started, under a policy with no `unsafe-eval` — which is the whole reason the
  CSP build is vendored, and the check that catches a regression to the CDN build.
- **The command palette** opens on `Ctrl`/`Cmd`+K, fills with entries the registry supplied,
  filters, and navigates where the chosen entry says.
- **The theme toggle** flips the root class and the choice survives a reload.
- **The skip link** is the first tab stop and points at `#main`.
- **The runner** is generated from the schema (the required string, the bounded integer, the
  disclosure of the capability's own benchmark cases), and submitting it calls the
  capability's real route and renders the answer — asserted against the request the browser
  actually made, not against the preview.
- **The graph** draws over the same nodes the page lists, or says why it cannot.
- **`frame-ancestors 'none'`** holds: another origin cannot frame the Cockpit.
- **JavaScript disabled**: every sampled route still carries its navigation and its content.

The probe measures the server the repository's lease names, because one server per
repository is the rule this tool lives by. When an editor holds the lease with an executable
older than the Cockpit, the probe says so and names what to do rather than quietly starting
a second server.

What it has already caught, which is the argument for it existing: Alpine's CSP build
treating `palette.open` as an expression it will not evaluate — leaving a full-page modal
backdrop over every click on a page that looked correct in the markup; the CDN build
starting itself before the component was registered; a topbar eight pixels too wide for a
390 px viewport on every page; and a table wrapper whose negative margin made the document
wider than the viewport.

Playwright drives the Chrome that is already installed (`channel: 'chrome'`), so nothing is
downloaded and CI reuses the browser the site probe already installs. A missing Chrome, a
missing Playwright or a missing Node is a SKIP of the browser sweep and not a failure — the
status sweep still runs over every route.

One consequence of reading assets once: a changed stylesheet or script needs the server
restarted before the probe sees it. That is deliberate — an asset is read from disk once per
process — and it is why the probe reports the digest it was served.
{% endraw %}
