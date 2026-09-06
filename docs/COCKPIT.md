# The Cockpit — the registry, rendered for a person

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

| Route | What it shows | Derived from |
|---|---|---|
| `/cockpit` | repository identity, git state, index state, the registry counted, every diagnostic, the health summary | `repository.info`, `system.health` |
| `/cockpit/capabilities` | every capability, filtered by module, kind, source or text | the registry |
| `/cockpit/capabilities/<id>` | one descriptor in full: schemas, projections, cache and benchmark policy, provenance, examples, and a form that runs it | the descriptor and its `BenchmarkCases` |
| `/cockpit/objects` | the declarative objects of the layer, by kind | `objects.list` |
| `/cockpit/object?uri=` | one object: front matter, provenance, content as it is | `objects.get` |
| `/cockpit/graphs` | every graph this executable derives | `graph.list` |
| `/cockpit/graphs/<id>` | one graph: the drawing, the vocabularies, and every node and edge as tables | `graph.get` |
| `/cockpit/graphs/topology` | the registry graph in three dimensions — optional | `graph.get` (`registry`) |
| `/cockpit/health` | one check per dimension, each with the engine that decided it and the command that reproduces it | `system.health` |
| `/cockpit/api` | every HTTP route the registry projects, and the projection's own | the registry |
| `/cockpit/search` | capabilities and objects matching one query | the registry, `objects.search` |
| `/cockpit/activity` | this process's counters and phases, and a running plot of them | `perf.counters` |
| `/cockpit/assets/<file>` | the stylesheet, the scripts, the vendored libraries | `share/cockpit/` |

Every route is a deep link: the filters are query parameters, a refresh loses nothing, and
a page can be sent to somebody.

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

| Graph | What it shows | Derived from |
|---|---|---|
| `registry` | modules, the capabilities they compose, the file each was declared in, and the MCP / HTTP / CLI projections | the registry |
| `layer` | every directory an object was discovered in, nested, with the kinds each holds | object provenance |
| `rules` | every rule and the rules it declares it depends on | `depends_on` front matter |
| `adrs` | every decision, what it supersedes, and the rules, claims, files and tests it put in force | ADR front matter |
| `use-cases` | every use case and the commands, doctrines and claims it names | use-case front matter |

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

`system.health` reports one check per dimension. Every check carries **who decided it** and
**what reproduces it**, because a health report that decides things itself is a fourth
opinion:

| Check | Decided by |
|---|---|
| the layer as it was read | the index's own diagnostics |
| the capability registry | the registry builder, which refuses to build on a duplicate or a malformed exposure |
| the declared scope | the scope declaration the index was built under |
| version control | git, asked once at startup |
| benchmark coverage | the benchmark projection's coverage — the same one `bench coverage --check` reads |
| committed projections | the same rendering `majordomus generate` writes, compared with what is committed |

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

| File | What it adds | Needs |
|---|---|---|
| `cockpit.js` | the one Alpine component (theme, palette state), the shared helpers | `vendor/alpine.csp.min.js` |
| `palette.js` | the command palette (`Ctrl`/`Cmd` + `K`), entries from the registry | — |
| `runner.js` | the generic capability runner | — |
| `graph.js` | the Cytoscape view: pan, zoom, fit, search, neighbourhood focus, a details drawer | `vendor/cytoscape.min.js` |
| `topology.js` | the registry graph in three dimensions | `vendor/three.module.min.js` |
| `activity.js` | a running plot of the execution and cache counters | `vendor/p5.min.js` |

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

| File | Committed? | Why |
|---|---|---|
| `cockpit.css` | yes | a generated artifact, compiled from `src/cockpit.css` with Tailwind and Flowbite's theme; drift-checked |
| `src/cockpit.css` | yes | the source: design tokens and one semantic class per component |
| `*.js`, `favicon.svg` | yes | written by hand, no build step |
| `vendor/alpine.csp.min.js` | yes | the interaction depends on it and it is 70 kB |
| `vendor/{cytoscape,three.module,three.core,p5}.min.js` | no | ~2 MB, lazy, optional, every page complete without them |

There are no utility classes in the Rust that renders the pages. Everything the markup
names is defined in `src/cockpit.css` as a composition of Tailwind utilities, which keeps
the design readable in one file and makes the compiled stylesheet a function of that file
and the pinned versions alone — which is what makes `--check` exact.

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
