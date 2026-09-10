<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the clap declaration in apps/majordomus-cli/src/cli.rs and the examples beside it; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.4.0 -->
# Command line of the Rust executable

Majordomus control plane: a data-driven MCP server over the repository's .ai/ layer

The Rust executable of Majordomus. It reads the repository's provider-neutral AI layer under .ai/ and serves it, read-only, to MCP clients over stdio.

The task lifecycle (init, start, check, finish, doctor, ...) is the shell tool bin/majordomus in the same repository; this executable does not implement those commands.

Every command below is declared once, in [`apps/majordomus-cli/src/cli.rs`](../../apps/majordomus-cli/src/cli.rs), together with its examples; this file is a projection of that declaration, as `--help` is, as `docs/generated/cli.json` is, and as the website's reference under `/docs/cli/` is. Every example printed here is executed against the built executable by `apps/majordomus-cli/tests/cli_examples.rs`. The task lifecycle (`init`, `start`, `check`, `finish`, `doctor`, ...) is the *shell* tool `bin/majordomus`, a different program, documented in `docs/CLI.md`.

## Commands

| command | route | does |
|---|---|---|
| [`majordomus mcp`](#majordomus-mcp) | `/docs/cli/mcp/` | Serve the repository's AI layer to an MCP client over stdio (read-only) |
| [`majordomus serve`](#majordomus-serve) | `/docs/cli/serve/` | Serve the same capabilities over HTTP on the loopback interface, with the home page, /openapi.json, /swagger and the documentation under /docs/ (read-only) |
| [`majordomus capabilities`](#majordomus-capabilities) | `/docs/cli/capabilities/` | Introspect the capability registry: what exists, where it came from, how it is exposed |
| [`majordomus capabilities list`](#majordomus-capabilities-list) | `/docs/cli/capabilities/list/` | Every capability, one line each, with its projections |
| [`majordomus capabilities describe`](#majordomus-capabilities-describe) | `/docs/cli/capabilities/describe/` | One capability by canonical id: schemas, provenance, every projection |
| [`majordomus capabilities schema`](#majordomus-capabilities-schema) | `/docs/cli/capabilities/schema/` | The canonical input or output JSON Schema of one capability |
| [`majordomus capabilities projections`](#majordomus-capabilities-projections) | `/docs/cli/capabilities/projections/` | Where each capability is projected, and every claim its surface does not answer |
| [`majordomus capabilities validate`](#majordomus-capabilities-validate) | `/docs/cli/capabilities/validate/` | Build the registry and every projection; exit 10 with every violation named |
| [`majordomus generate`](#majordomus-generate) | `/docs/cli/generate/` | Write the committed projections of the registry (docs/generated), or check that they are current |
| [`majordomus bench`](#majordomus-bench) | `/docs/cli/bench/` | Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations), report coverage, compare with the accepted baseline |
| [`majordomus bench coverage`](#majordomus-bench-coverage) | `/docs/cli/bench/coverage/` | Every required target and whether it is covered; the denominator is generated from the registry |
| [`majordomus bench baseline`](#majordomus-bench-baseline) | `/docs/cli/bench/baseline/` | The accepted baseline of this platform under .ai/repo/benchmarks/rust/ |
| [`majordomus bench baseline update`](#majordomus-bench-baseline-update) | `/docs/cli/bench/baseline/update/` | Run the benchmarks and record them as this platform's baseline (a reviewable, tracked file) |
| [`majordomus scope`](#majordomus-scope) | `/docs/cli/scope/` | The repository scope: what a worker reads and what it never reads; with paths, whether each is in or out and why |
| [`majordomus web`](#majordomus-web) | `/docs/cli/web/` | The repository's web surfaces: what is exposed, where it is mounted, what produced it, and whether the topology is valid |
| [`majordomus web list`](#majordomus-web-list) | `/docs/cli/web/list/` | Every discovered surface: id, kind, mount, producer |
| [`majordomus web explain`](#majordomus-web-explain) | `/docs/cli/web/explain/` | Why each surface exists and where each of its values came from |
| [`majordomus web validate`](#majordomus-web-validate) | `/docs/cli/web/validate/` | Check the topology's invariants; exit 10 on any error finding |
| [`majordomus web manifest`](#majordomus-web-manifest) | `/docs/cli/web/manifest/` | Write the resolved topology to the generated manifest |
| [`majordomus web report`](#majordomus-web-report) | `/docs/cli/web/report/` | Render a generated report into its own surface under the generated web root |
| [`majordomus web report tests`](#majordomus-web-report-tests) | `/docs/cli/web/report/tests/` | The test run: the behavioural cases' report, and the crate's own totals |
| [`majordomus web report benchmarks`](#majordomus-web-report-benchmarks) | `/docs/cli/web/report/benchmarks/` | The benchmark run: a results document, or the accepted baseline |
| [`majordomus web report ui`](#majordomus-web-report-ui) | `/docs/cli/web/report/ui/` | The UI conformance audit, rendered as a section of the test surface (/tests/ui) |
| [`majordomus web compose`](#majordomus-web-compose) | `/docs/cli/web/compose/` | Compose every published surface into one publishable tree |
| [`majordomus why`](#majordomus-why) | `/docs/cli/why/` | The operational moments this tool answers: the catalogue, one moment, the audiences and areas, a diagnosis of your own week, and the catalogue's own validation |
| [`majordomus why list`](#majordomus-why-list) | `/docs/cli/why/list/` | Every operational moment, narrowed by any facet the catalogue reports |
| [`majordomus why show`](#majordomus-why-show) | `/docs/cli/why/show/` | One moment in full, with every relation derived from its metadata |
| [`majordomus why audiences`](#majordomus-why-audiences) | `/docs/cli/why/audiences/` | Every audience, with the moments that name it |
| [`majordomus why areas`](#majordomus-why-areas) | `/docs/cli/why/areas/` | Every operational area, with the moments that fall under it |
| [`majordomus why diagnose`](#majordomus-why-diagnose) | `/docs/cli/why/diagnose/` | What the symptoms you recognise imply: the areas they weigh towards and the mechanisms that answer them |
| [`majordomus why validate`](#majordomus-why-validate) | `/docs/cli/why/validate/` | Every finding over the catalogue; exit 10 when any is an error |
| [`majordomus distribution`](#majordomus-distribution) | `/docs/cli/distribution/` | How this project is packaged, published and installed: the platforms, the artifact names, the installer, the releases |
| [`majordomus distribution show`](#majordomus-distribution-show) | `/docs/cli/distribution/show/` | The model: the install command, where an installation goes, and every declared target |
| [`majordomus distribution status`](#majordomus-distribution-status) | `/docs/cli/distribution/status/` | Whether the advertised one-line installation works right now, and what is missing when it does not |
| [`majordomus distribution validate`](#majordomus-distribution-validate) | `/docs/cli/distribution/validate/` | Every invariant of the model and of the release records; exit 10 with each violation named |
| [`majordomus distribution targets`](#majordomus-distribution-targets) | `/docs/cli/distribution/targets/` | Every declared target, one line each, with the artifact name it derives |
| [`majordomus distribution matrix`](#majordomus-distribution-matrix) | `/docs/cli/distribution/matrix/` | The release build matrix, as the release workflow reads it |
| [`majordomus distribution artifact`](#majordomus-distribution-artifact) | `/docs/cli/distribution/artifact/` | The archive name and root directory a target and a tag derive |
| [`majordomus distribution releases`](#majordomus-distribution-releases) | `/docs/cli/distribution/releases/` | Every recorded release, newest first, and the one an unpinned installation resolves to |
| [`majordomus distribution metadata`](#majordomus-distribution-metadata) | `/docs/cli/distribution/metadata/` | The public metadata one release record publishes, rendered from the record alone |
| [`majordomus distribution build`](#majordomus-distribution-build) | `/docs/cli/distribution/build/` | What this executable is: version, target triple, profile, commit |
| [`majordomus env`](#majordomus-env) | `/docs/cli/env/` | What this checkout is: the project, version control, the toolchains it declares, what the layer holds, the workflows, the provider projections and the local services |
| [`majordomus env status`](#majordomus-env-status) | `/docs/cli/env/status/` | The whole snapshot, resolved in full: what the layer holds is counted, and the cache the banner reads is written |
| [`majordomus env banner`](#majordomus-env-banner) | `/docs/cli/env/banner/` | Render the snapshot for a terminal. Goes to standard error, never standard output, because direnv reads standard output as the environment it is setting |
| [`majordomus env export`](#majordomus-env-export) | `/docs/cli/env/export/` | The variable assignments a shell in this repository benefits from, for `eval`. Assignments only: no command, no side effect |
| [`majordomus env explain`](#majordomus-env-explain) | `/docs/cli/env/explain/` | Where each value came from: the file, command or constant that decided it, the resolver that read it, and how far it can be trusted |
| [`majordomus commands`](#majordomus-commands) | `/docs/cli/commands/` | Every command this repository offers, from whichever program offers it: the graph, one command, where each one is projected, and the workflow bridge derived from it |
| [`majordomus commands list`](#majordomus-commands-list) | `/docs/cli/commands/list/` | Every command, one line each: what it is, what running it changes, and where it is projected |
| [`majordomus commands show`](#majordomus-commands-show) | `/docs/cli/commands/show/` | One command in full: its arguments, its effect, what it needs, and every surface that carries it |
| [`majordomus commands explain`](#majordomus-commands-explain) | `/docs/cli/commands/explain/` | Why one command appears where it does: the declaration it came from, the policy that placed it, and the reason for every surface that withholds it |
| [`majordomus commands graph`](#majordomus-commands-graph) | `/docs/cli/commands/graph/` | The whole graph as one document, with its fingerprint and every diagnostic |
| [`majordomus commands bridge`](#majordomus-commands-bridge) | `/docs/cli/commands/bridge/` | Materialise the workflow bridge from the graph, and refresh the cache the completion reads; writes nothing when the graph has not changed |
| [`majordomus completion`](#majordomus-completion) | `/docs/cli/completion/` | Completion for any surface, answered from the command graph: the candidates a shell asks for, and the one-time integration that asks |
| [`majordomus completion query`](#majordomus-completion-query) | `/docs/cli/completion/query/` | The candidates for one command line, from the command graph. What a shell adapter calls on every TAB |
| [`majordomus completion init`](#majordomus-completion-init) | `/docs/cli/completion/init/` | The shell integration to load once, which carries no command of its own and asks this executable for every candidate |
| [`majordomus completion install`](#majordomus-completion-install) | `/docs/cli/completion/install/` | Put that integration into the shell's startup file, between managed markers, so that no one maintains it by hand |
| [`majordomus worktree`](#majordomus-worktree) | `/docs/cli/worktree/` | The branch-to-worktree topology: where every linked worktree belongs (`<repo>-wt/<branch>`), where each one is, and the lifecycle — create, migrate, repair, guard |
| [`majordomus worktree status`](#majordomus-worktree-status) | `/docs/cli/worktree/status/` | Where this call is — branch, worktree, canonical or not, uncommitted work — and how many errors the whole topology carries; exit 10 when this worktree is out of place |
| [`majordomus worktree list`](#majordomus-worktree-list) | `/docs/cli/worktree/list/` | Every registered worktree with its standing, one line each; exit 10 when the topology has an error |
| [`majordomus worktree topology`](#majordomus-worktree-topology) | `/docs/cli/worktree/topology/` | The whole topology: repository, container, trunk, every worktree, every branch without a worktree, every diagnostic; exit 10 when it has an error |
| [`majordomus worktree root`](#majordomus-worktree-root) | `/docs/cli/worktree/root/` | Print the container every linked worktree belongs under, and nothing else: `cd "$(majordomus worktree root)"` |
| [`majordomus worktree path`](#majordomus-worktree-path) | `/docs/cli/worktree/path/` | Print the canonical path of a branch, and nothing else: `cd "$(majordomus worktree path feature/x)"`. Derived from the name; the branch need not exist |
| [`majordomus worktree inspect`](#majordomus-worktree-inspect) | `/docs/cli/worktree/inspect/` | One branch: its canonical path, whether it exists, what occupies the path, the worktree holding it, and what stands in the way |
| [`majordomus worktree create`](#majordomus-worktree-create) | `/docs/cli/worktree/create/` | Create the canonical worktree of a branch, creating the branch from --base (default: the trunk) when it does not exist. The path is derived; none may be given |
| [`majordomus worktree ensure`](#majordomus-worktree-ensure) | `/docs/cli/worktree/ensure/` | The canonical worktree of a branch: created when absent, answered when present, refused when the branch is checked out somewhere else |
| [`majordomus worktree migrate`](#majordomus-worktree-migrate) | `/docs/cli/worktree/migrate/` | Bring every misplaced worktree to its canonical path, dirty state included, with a fingerprint taken before and after each move; --plan shows the steps and changes nothing |
| [`majordomus worktree validate`](#majordomus-worktree-validate) | `/docs/cli/worktree/validate/` | Every error of the topology, and nothing else; exit 10 when there is one |
| [`majordomus worktree doctor`](#majordomus-worktree-doctor) | `/docs/cli/worktree/doctor/` | Every diagnostic of the topology, errors, warnings and facts, each with its code and remedy; exit 10 when there is an error |
| [`majordomus worktree guard`](#majordomus-worktree-guard) | `/docs/cli/worktree/guard/` | May a mutation proceed from here? Exit 0 in a canonical worktree, in the primary checkout on the trunk, or detached; exit 10 with the reason otherwise. What the pre-commit hook asks |
| [`majordomus worktree repair`](#majordomus-worktree-repair) | `/docs/cli/worktree/repair/` | Drop git's registrations of worktrees whose directories are gone, and repair the administrative links of the ones that exist. Deletes no directory |
| [`majordomus worktree remove`](#majordomus-worktree-remove) | `/docs/cli/worktree/remove/` | Remove one linked worktree by branch or path. Never the primary checkout, never a branch, never uncommitted work without --force |
| [`majordomus worktree cleanup`](#majordomus-worktree-cleanup) | `/docs/cli/worktree/cleanup/` | The branches merged into the trunk whose worktree is clean or absent: what could be removed. Removes nothing |
| [`majordomus worktree branches`](#majordomus-worktree-branches) | `/docs/cli/worktree/branches/` | Every local branch, one per line, for a shell completion that wants the live set |
| [`majordomus product`](#majordomus-product) | `/docs/cli/product/` | The product: what this repository's tool does for a person, as the features under the layer declare it, with every surface, count and moment derived; the matrix of features against interfaces; the providers; and the model's own validation |
| [`majordomus product list`](#majordomus-product-list) | `/docs/cli/product/list/` | Every feature, narrowed by any filter, with the surfaces derived for each |
| [`majordomus product show`](#majordomus-product-show) | `/docs/cli/product/show/` | One feature in full: what it is made of, resolved, and everything derived from that |
| [`majordomus product matrix`](#majordomus-product-matrix) | `/docs/cli/product/matrix/` | Every feature against every interface, and every module, command and kind against the features that name it |
| [`majordomus product providers`](#majordomus-product-providers) | `/docs/cli/product/providers/` | Every provider the tool has an adapter for, with what this repository does with it |
| [`majordomus product validate`](#majordomus-product-validate) | `/docs/cli/product/validate/` | Every finding over the model; exit 10 when any is an error |
| [`majordomus release`](#majordomus-release) | `/docs/cli/release/` | What this project has shipped and what it would ship next: the changelog derived from the layer's own records, the version the two writers state, and the one command that raises both |
| [`majordomus release changelog`](#majordomus-release-changelog) | `/docs/cli/release/changelog/` | The changelog, composed from the layer's release records, the decisions dated inside each release's window, and the conventional commits in its range |
| [`majordomus release version`](#majordomus-release-version) | `/docs/cli/release/version/` | The version the two writers state, whether they agree, and the bump the commits since the last release imply |
| [`majordomus release bump`](#majordomus-release-bump) | `/docs/cli/release/bump/` | Raise the version in both places at once, to the bump the commits imply or to one you name |
| [`majordomus quality`](#majordomus-quality) | `/docs/cli/quality/` | What this executable's own public surface is held to: documentation, executable examples, module coverage, and every command accounted for against the capability registry |
| [`majordomus quality report`](#majordomus-quality-report) | `/docs/cli/quality/report/` | Measure the crate and report every finding, with the rule it breaks and what to do about it |
| [`majordomus run`](#majordomus-run) | `/docs/cli/run/` | Run a capability as an execution and follow it: its steps, its progress and its output as they happen |
| [`majordomus executions`](#majordomus-executions) | `/docs/cli/executions/` | The executions of the server serving this repository: what has run, what is running, and what each one said |
| [`majordomus executions list`](#majordomus-executions-list) | `/docs/cli/executions/list/` | Every execution the server remembers, newest first |
| [`majordomus executions show`](#majordomus-executions-show) | `/docs/cli/executions/show/` | One execution in full: its state, its steps, its diagnostics and what it produced |
| [`majordomus executions events`](#majordomus-executions-events) | `/docs/cli/executions/events/` | One execution's retained events, oldest first |
| [`majordomus executions cancel`](#majordomus-executions-cancel) | `/docs/cli/executions/cancel/` | Ask an execution to stop |
| [`majordomus executions protocol`](#majordomus-executions-protocol) | `/docs/cli/executions/protocol/` | The live channel's contract: where it is, what it writes, and the schema of each message |

<a id="majordomus"></a>
## `majordomus`

Majordomus control plane: a data-driven MCP server over the repository's .ai/ layer

Subcommands: [`majordomus mcp`](#majordomus-mcp), [`majordomus serve`](#majordomus-serve), [`majordomus capabilities`](#majordomus-capabilities), [`majordomus generate`](#majordomus-generate), [`majordomus bench`](#majordomus-bench), [`majordomus scope`](#majordomus-scope), [`majordomus web`](#majordomus-web), [`majordomus why`](#majordomus-why), [`majordomus distribution`](#majordomus-distribution), [`majordomus env`](#majordomus-env), [`majordomus commands`](#majordomus-commands), [`majordomus completion`](#majordomus-completion), [`majordomus worktree`](#majordomus-worktree), [`majordomus product`](#majordomus-product), [`majordomus release`](#majordomus-release), [`majordomus quality`](#majordomus-quality), [`majordomus run`](#majordomus-run), [`majordomus executions`](#majordomus-executions).

```text
majordomus <COMMAND>
```

Arguments: none.

<a id="majordomus-mcp"></a>
## `majordomus mcp`

Serve the repository's AI layer to an MCP client over stdio (read-only)

```text
majordomus mcp [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--inspect` | flag | — | Print what would be served, and every diagnostic, then exit without serving |
| `--format` | `text` \| `json` | `text` | Output shape of --inspect — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--transport` | `stdio` | `stdio` | The transport to serve on — `stdio`: One JSON-RPC frame per line on stdin and stdout |
| `--standalone` | flag | — | Serve this client alone: no shared server, no HTTP, no Swagger UI, no peers, and nothing written anywhere. The default is the shared server (below) |
| `--http-host` | `<HOST>` | `127.0.0.1` | Interface the shared server binds when this process is the one that starts it |
| `--http-port` | `<PORT>` | `8741` | Port the shared server binds when this process starts it; when it is taken, a free port is used instead and the URL is logged on stderr either way |

Examples:

- **See what would be served, without serving it** — Builds the registry and the index of the repository in the working directory and prints the repository, the capabilities, the objects and every diagnostic, then exits. Nothing is served and nothing is written.

  ```console
  $ majordomus mcp --inspect
  ```

  Verified: exits 0; prints repository, capabilities.

- **The same, as one JSON document for a script** — The shape `--inspect` prints for a person, as JSON: the repository, its discovery mode, the capabilities and the diagnostics, deterministic and safe to diff.

  ```console
  $ majordomus mcp --inspect --format json
  ```

  Verified: exits 0; prints one JSON document carrying /repository/repository/root, /tools.

- **Serve one MCP client on stdio** — The form an MCP client spawns: JSON-RPC frames in on stdin, frames out on stdout, logs on stderr, and the session ends at end of input. `--standalone` keeps this process to itself: no shared server, no HTTP, nothing written anywhere.

  ```console
  $ majordomus mcp --standalone
  ```

  Verified: answers initialize and tools/list on stdio, and exits 0 at end of input.

<a id="majordomus-serve"></a>
## `majordomus serve`

Serve the same capabilities over HTTP on the loopback interface, with the home page, /openapi.json, /swagger and the documentation under /docs/ (read-only)

```text
majordomus serve [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--host` | `<HOST>` | `127.0.0.1` | Interface to bind; loopback unless you say otherwise |
| `--port` | `<PORT>` | `8741` | Port to bind; 0 picks a free one and the address is logged on stderr |
| `--deployment` | `<ID>` | — | Bind the address this deployment object declares (`.ai/repo/deployments/<ID>.yaml`) instead of the local default. What a hosted process is started with; the address is the object's, not this command line's |

Examples:

- **Serve the same capabilities over HTTP on a free port** — Port 0 asks the operating system for a free port; the address is logged on stderr. `/` is the home page, generated from the surfaces this process resolved; the document at /openapi.json is the same one `majordomus generate` commits; /swagger is the Swagger UI over it; /docs/ is this repository's documentation when it has been built for that mount.

  ```console
  $ majordomus serve --port 0
  ```

  Verified: binds a port, answers GET /openapi.json, exits 0 when stopped.

<a id="majordomus-capabilities"></a>
## `majordomus capabilities`

Introspect the capability registry: what exists, where it came from, how it is exposed

Subcommands: [`majordomus capabilities list`](#majordomus-capabilities-list), [`majordomus capabilities describe`](#majordomus-capabilities-describe), [`majordomus capabilities schema`](#majordomus-capabilities-schema), [`majordomus capabilities projections`](#majordomus-capabilities-projections), [`majordomus capabilities validate`](#majordomus-capabilities-validate).

```text
majordomus capabilities [OPTIONS] <COMMAND>
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

<a id="majordomus-capabilities-list"></a>
## `majordomus capabilities list`

Every capability, one line each, with its projections

```text
majordomus capabilities list [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--kind` | `<KIND>` | — | Only this kind: query or resource |
| `--exposure` | `<EXPOSURE>` | — | Only capabilities exposed through this projection: mcp, http or cli |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **Which capabilities the command line itself dispatches to** — One line per capability exposed through the `cli` projection, with the projections of each. The registry answers this; no list of capabilities is written in the command line's own declaration.

  ```console
  $ majordomus capabilities list --exposure cli
  ```

  Verified: exits 0; prints capabilities.list, capabilities.describe.

- **Every capability as one JSON document** — The whole registry for a script: each capability with its kind, its provenance and every projection it has.

  ```console
  $ majordomus capabilities list --format json
  ```

  Verified: exits 0; prints one JSON document carrying /capabilities.

<a id="majordomus-capabilities-describe"></a>
## `majordomus capabilities describe`

One capability by canonical id: schemas, provenance, every projection

```text
majordomus capabilities describe [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The canonical id |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **One capability in full, by its canonical id** — Its kind, its input and output schemas, where it was composed, and every projection of it: the MCP tool or resource, the HTTP route, the CLI path.

  ```console
  $ majordomus capabilities describe objects.get
  ```

  Verified: exits 0; prints objects.get, GET /api/v1/object.

<a id="majordomus-capabilities-schema"></a>
## `majordomus capabilities schema`

The canonical input or output JSON Schema of one capability

```text
majordomus capabilities schema [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The canonical id |
| `--side` | `input` \| `output` | `input` | Input or output — `input`: The schema of the input; `output`: The schema of the output |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **The canonical output schema of a capability** — The JSON Schema the MCP and OpenAPI projections are derived from; `--side input` prints the schema of what the capability accepts.

  ```console
  $ majordomus capabilities schema objects.get --side output
  ```

  Verified: exits 0; prints one JSON document carrying /title.

<a id="majordomus-capabilities-projections"></a>
## `majordomus capabilities projections`

Where each capability is projected, and every claim its surface does not answer

```text
majordomus capabilities projections [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--module` | `<MODULE>` | — | Only capabilities composed in this module |
| `--unmet` | flag | — | Only the capabilities whose declared exposures are not all answered |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **Every exposure a capability claims that its surface does not answer** — `rows: 0` is the closure `project.interfaces-are-projections` asks for: every declared command line, route and tool is answered by the surface that carries it. The commands no capability claims are reported beside it, as the measure of how much of the command line is still hand-written.

  ```console
  $ majordomus capabilities projections --unmet
  ```

  Verified: exits 0.

- **Where one module's capabilities appear** — A row per capability with the command line, HTTP route and MCP tool it reaches, so a capability that exists but is reachable from nowhere is visible as one.

  ```console
  $ majordomus capabilities projections --module worktree
  ```

  Verified: exits 0; prints worktree.topology, majordomus worktree topology.

<a id="majordomus-capabilities-validate"></a>
## `majordomus capabilities validate`

Build the registry and every projection; exit 10 with every violation named

```text
majordomus capabilities validate [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **Prove the registry and every projection of it** — Builds the registry, the MCP and HTTP surfaces, the OpenAPI document, the command line's documentation and the benchmark coverage, and names every failure. Exit 10 when anything is unmet.

  ```console
  $ majordomus capabilities validate
  ```

  Verified: exits 0; prints validate: 0 failure(s), OK   cli.

<a id="majordomus-generate"></a>
## `majordomus generate`

Write the committed projections of the registry (docs/generated), or check that they are current

```text
majordomus generate [OPTIONS] [TARGET]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `<TARGET>` | `all` \| `openapi` \| `docs` \| `benchmarks` \| `registry` \| `allow` \| `providers` \| `site` \| `manifest` \| `distribution` \| `web` \| `changelog` \| `deployment` \| `design` | `all` | What to generate — `all`: Every target; `openapi`: `docs/generated/openapi.{json,yaml}`; `docs`: `docs/generated/capabilities.md`, `docs/generated/modules/<id>.md` and `docs/generated/cli.{md,json,yaml}`; `benchmarks`: `docs/generated/benchmarks.{md,json,yaml}`: every benchmark target and the coverage; `registry`: `docs/generated/registry.{json,yaml}`: the builtin registry as data; `allow`: The shell tool's allow-lists under share/allow, derived from the schemas; `providers`: The provider bootstraps the policy declares (AGENTS.md, CLAUDE.md, ...); `site`: site/data/registry/registry.json, the registry dataset the site renders; `manifest`: docs/generated/artifacts.{json,yaml,md}: the index of every generated artifact; `distribution`: The installer, the installation guide, the release build matrix and the public release metadata, from share/distribution.yaml and .ai/repo/releases/; `web`: `docs/generated/web.json`: the resolved web topology the site's route reference renders; `changelog`: `docs/generated/changelog.{json,yaml,md}`: the changelog composed from the layer's release records, its decisions and the repository's commits; `deployment`: deploy/Dockerfile, .dockerignore and fly.toml, from the deployment objects; `design`: The design system's projections, from share/design/tokens.yaml: the stylesheets both Tailwind builds import, the tokens and the declaration compiled into the crate, every copy of the brand, site/data/registry/design.json and docs/generated/design.* |
| `--check` | flag | — | Compare with what is on disk and exit 10 when stale; write nothing |
| `--out` | `<DIR>` | — | Write under this directory instead of the repository root (docs/generated is appended) |

Examples:

- **Write every committed projection** — The OpenAPI document, the capability reference, the command-line reference and its JSON, the registry manifest, the benchmark matrix, the shell tool's allow-lists, the provider bootstraps and the site's registry dataset — all from the one registry and the one clap declaration.

  ```console
  $ majordomus generate
  ```

  Verified: exits 0.

- **Refuse a tree whose projections are stale** — Writes nothing and compares instead: exit 0 when every committed projection is what the sources produce, exit 10 with each stale file named. This is the form CI runs.

  ```console
  $ majordomus generate
  $ majordomus generate --check
  ```

  Verified: exits 0.

- **One target only** — Each target can be written on its own while a change is iterated on; `majordomus generate` with no target writes all of them.

  ```console
  $ majordomus generate openapi
  ```

  Verified: exits 0.

<a id="majordomus-bench"></a>
## `majordomus bench`

Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations), report coverage, compare with the accepted baseline

Subcommands: [`majordomus bench coverage`](#majordomus-bench-coverage), [`majordomus bench baseline`](#majordomus-bench-baseline).

```text
majordomus bench [OPTIONS] [COMMAND] [ID]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `<ID>` | `<ID>` | — | Only targets of this capability id, or whose key starts with this text |
| `--transport` | `all` \| `direct` \| `mcp` \| `http` \| `system` | `all` | Only this transport — `all`: Every target; `direct`: Capabilities through the executor, in process; `mcp`: Capabilities through a real `majordomus mcp` child; `http`: Capabilities over a real loopback socket; `system`: The transports' own operations only |
| `--profile` | `quick` \| `full` \| `ci` | `quick` | How much to measure — `quick`: Fast developer feedback: few samples; `full`: Stable evidence: many samples, many cold spawns; `ci`: Conservative: structural gates plus a modest measurement |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--check` | flag | — | Compare with the accepted baseline of this platform under .ai/repo/benchmarks/rust/policy.yaml; exit 10 on a regression |
| `--no-write` | flag | — | Do not write the result under .ai/local/benchmarks/ |

Examples:

- **Time the capabilities in process** — The quick profile takes few samples, and `--transport direct` measures the executor without spawning a server. Nothing is written under .ai/local/ with `--no-write`.

  ```console
  $ majordomus bench --transport direct --profile quick --no-write --format json
  ```

  Verified: exits 0; prints one JSON document carrying /results, /profile.

<a id="majordomus-bench-coverage"></a>
## `majordomus bench coverage`

Every required target and whether it is covered; the denominator is generated from the registry

```text
majordomus bench coverage [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--check` | flag | — | Exit 10 when any required target is missing or waived |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **Every required benchmark target and whether it is covered** — The denominator is generated from the registry: every executable capability, on every transport it is exposed on, plus the transports' own operations.

  ```console
  $ majordomus bench coverage --format json
  ```

  Verified: exits 0; prints one JSON document carrying /lines, /tallies.

- **Fail when a target is missing** — Exit 10 when any required target is uncovered or waived, so a capability that nothing times cannot be merged.

  ```console
  $ majordomus bench coverage --check
  ```

  Verified: exits 0.

<a id="majordomus-bench-baseline"></a>
## `majordomus bench baseline`

The accepted baseline of this platform under .ai/repo/benchmarks/rust/

Subcommands: [`majordomus bench baseline update`](#majordomus-bench-baseline-update).

```text
majordomus bench baseline [OPTIONS] <COMMAND>
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

<a id="majordomus-bench-baseline-update"></a>
## `majordomus bench baseline update`

Run the benchmarks and record them as this platform's baseline (a reviewable, tracked file)

```text
majordomus bench baseline update [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--profile` | `quick` \| `full` \| `ci` | `full` | How much to measure — `quick`: Fast developer feedback: few samples; `full`: Stable evidence: many samples, many cold spawns; `ci`: Conservative: structural gates plus a modest measurement |
| `--allow-dirty` | flag | — | Record even from a dirty work tree |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **Record this platform's accepted baseline** — Runs the benchmarks and writes the result under .ai/repo/benchmarks/rust/ as a tracked, reviewable file. The full profile is the default; the quick profile is for trying the path out. A dirty work tree is refused unless --allow-dirty says otherwise.

  ```console
  $ majordomus bench baseline update --profile quick --allow-dirty
  ```

  Verified: exits 0.

<a id="majordomus-scope"></a>
## `majordomus scope`

The repository scope: what a worker reads and what it never reads; with paths, whether each is in or out and why

```text
majordomus scope [OPTIONS] [PATHS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `<PATHS>` | `<PATHS>` | — | Repository-relative paths to judge; none prints the declaration and the tally |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--check` | flag | — | Exit 10 when any path given is out of the scope |

Examples:

- **What a worker reads of this repository** — With no path, the declaration itself and the tally: how many tracked files are in the scope and how many are out.

  ```console
  $ majordomus scope
  ```

  Verified: exits 0.

- **Judge paths, and say which rule decided** — For each path: in or out, and the rule that decided it. `--check` exits 10 when any path given is out, which is how a hook refuses to read one.

  ```console
  $ majordomus scope docs/CLI.md --format json
  ```

  Verified: exits 0; prints one JSON document carrying /0/verdict, /0/rule.

<a id="majordomus-web"></a>
## `majordomus web`

The repository's web surfaces: what is exposed, where it is mounted, what produced it, and whether the topology is valid

Subcommands: [`majordomus web list`](#majordomus-web-list), [`majordomus web explain`](#majordomus-web-explain), [`majordomus web validate`](#majordomus-web-validate), [`majordomus web manifest`](#majordomus-web-manifest), [`majordomus web report`](#majordomus-web-report), [`majordomus web compose`](#majordomus-web-compose).

```text
majordomus web [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **The topology, from the command with no subcommand** — `web` with nothing after it lists, because listing is what a person wants when they ask what this repository exposes.

  ```console
  $ majordomus web
  ```

  Verified: exits 0; prints ID, MOUNT.

<a id="majordomus-web-list"></a>
## `majordomus web list`

Every discovered surface: id, kind, mount, producer

```text
majordomus web list [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Every web surface this repository exposes** — The resolved topology, in route-precedence order: the routes the executable answers itself, the application's site, and every generated report that declared itself under the generated web root. Nothing is registered anywhere; each line was discovered.

  ```console
  $ majordomus web list
  ```

  Verified: exits 0; prints MOUNT, /api/v1, /swagger.

<a id="majordomus-web-explain"></a>
## `majordomus web explain`

Why each surface exists and where each of its values came from

```text
majordomus web explain [OPTIONS] [ID]
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | — | Only this surface; none explains every one |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Why a surface exists and where each of its values came from** — For each field a reader could be surprised by — the mount, the kind, the directory — the source that decided it: a producer's own declaration, the site configuration, the capability registry, or the model's documented default.

  ```console
  $ majordomus web explain swagger
  ```

  Verified: exits 0; prints swagger, came from.

<a id="majordomus-web-validate"></a>
## `majordomus web validate`

Check the topology's invariants; exit 10 on any error finding

```text
majordomus web validate [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--artifacts` | flag | — | Also require every static surface's directory and index to exist |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Check the topology before anything serves or publishes it** — Two surfaces claiming one path, a surface nested inside another's subtree, a directory outside the generated root or one that walks out of the repository: each is a named finding with the surface, the value, its source and the fix. Exit 10 on any error finding. `--artifacts` also requires every static surface's directory and index to exist, which is what serving and publishing need.

  ```console
  $ majordomus web validate
  ```

  Verified: exits 0; prints no conflict.

<a id="majordomus-web-manifest"></a>
## `majordomus web manifest`

Write the resolved topology to the generated manifest

```text
majordomus web manifest [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Write the resolved topology down for another tool to read** — The manifest under the generated web root is derived state: a publisher or a CI job may read it instead of resolving the topology again, and nothing may edit it, because the next run overwrites it from the same discovery.

  ```console
  $ majordomus web manifest
  ```

  Verified: exits 0; prints web manifest, surface.

<a id="majordomus-web-report"></a>
## `majordomus web report`

Render a generated report into its own surface under the generated web root

Subcommands: [`majordomus web report tests`](#majordomus-web-report-tests), [`majordomus web report benchmarks`](#majordomus-web-report-benchmarks), [`majordomus web report ui`](#majordomus-web-report-ui).

```text
majordomus web report [OPTIONS] <COMMAND>
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

<a id="majordomus-web-report-tests"></a>
## `majordomus web report tests`

The test run: the behavioural cases' report, and the crate's own totals

```text
majordomus web report tests [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--suite` | `<SUITE>` | required | The runner's TSV report (`MJ_TEST_REPORT=<file> bash test/run.sh`) |
| `--crate-output` | `<CRATE_OUTPUT>` | — | The output of `cargo test`, for its totals |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Render the suite's own results into the /tests surface** — The runner writes its report with `MJ_TEST_REPORT=<file> bash test/run.sh`; this renders it, keeps the machine-readable results beside the page, and declares the directory so discovery finds it. Without that file there is nothing to render and the command says so rather than publishing an empty page.

  ```console
  $ majordomus web report tests --suite target/web/run.tsv
  ```

  Verified: exits 13.

<a id="majordomus-web-report-benchmarks"></a>
## `majordomus web report benchmarks`

The benchmark run: a results document, or the accepted baseline

```text
majordomus web report benchmarks [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--from` | `<FROM>` | required | A results document from `majordomus bench`, or a baseline under the layer |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Render a benchmark run into the /benchmarks surface** — Reads a results document — a run's own output, or an accepted baseline under the layer's benchmarks section, which have the same shape — and renders every measured target ordered by median. It measures nothing itself: a figure on the page is a figure a run produced.

  ```console
  $ majordomus web report benchmarks --from .ai/repo/benchmarks/rust/baseline.macos-aarch64-debug.json
  ```

  Verified: exits 13.

<a id="majordomus-web-report-ui"></a>
## `majordomus web report ui`

The UI conformance audit, rendered as a section of the test surface (/tests/ui)

```text
majordomus web report ui [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--from` | `<FROM>` | required | A results document from `scripts/ui audit` |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Render the UI conformance audit into /tests/ui** — `scripts/ui audit` drives a browser over every page of the built site at every width the compiled stylesheet's breakpoints imply, and writes one results document; this renders it. The rendering is a section of the test surface rather than a surface of its own, because a conformance run is a test run and the topology refuses a surface mounted inside another's subtree. Without that document there is nothing to render and the command says so.

  ```console
  $ majordomus web report ui --from target/web/run-ui.json
  ```

  Verified: exits 13.

<a id="majordomus-web-compose"></a>
## `majordomus web compose`

Compose every published surface into one publishable tree

```text
majordomus web compose [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--destination` | `<DESTINATION>` | — | Where to write it; the default is target/site |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--only` | `<ONLY>` | — | Only these surfaces, by discovered id (repeat or separate with commas) (accepted by every subcommand) |
| `--exclude` | `<EXCLUDE>` | — | Every surface except these, by discovered id (accepted by every subcommand) |

Examples:

- **Compose every published surface into one publishable tree** — Each producer owns its own output directory; publication needs one tree, and the mapping is the resolved mount and nothing else. A surface that is discovered is published without a copy step being written anywhere, and a repository with nothing generated yet composes an empty tree rather than an error. Where a surface exists and its directory does not, composition refuses and names the producer to run.

  ```console
  $ majordomus web compose --destination target/site
  ```

  Verified: exits 0.

<a id="majordomus-why"></a>
## `majordomus why`

The operational moments this tool answers: the catalogue, one moment, the audiences and areas, a diagnosis of your own week, and the catalogue's own validation

Subcommands: [`majordomus why list`](#majordomus-why-list), [`majordomus why show`](#majordomus-why-show), [`majordomus why audiences`](#majordomus-why-audiences), [`majordomus why areas`](#majordomus-why-areas), [`majordomus why diagnose`](#majordomus-why-diagnose), [`majordomus why validate`](#majordomus-why-validate).

```text
majordomus why [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--audience` | `<AUDIENCE>` | — | Only moments this audience recognises (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only moments in this operational area (accepted by every subcommand) |
| `--tag` | `<TAG>` | — | Only moments carrying this tag (accepted by every subcommand) |
| `--severity` | `<SEVERITY>` | — | Only moments of this severity (accepted by every subcommand) |
| `--frequency` | `<FREQUENCY>` | — | Only moments of this frequency (accepted by every subcommand) |
| `--lifecycle` | `<LIFECYCLE>` | — | Only moments at this stage of work (accepted by every subcommand) |
| `--capability` | `<CAPABILITY>` | — | Only moments naming this capability of the executable (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only moments naming this command (accepted by every subcommand) |
| `--featured` | flag | — | Only the moments the homepage features (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated moments, not only the public ones (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies (accepted by every subcommand) |

Examples:

- **The operational moments this repository holds** — `why` with nothing after it lists, because listing is what a person wants when they ask what this section is. The count on the last line is computed from the catalogue; no number anywhere is written down.

  ```console
  $ majordomus why
  ```

  Verified: exits 0; prints SLUG, moment(s).

<a id="majordomus-why-list"></a>
## `majordomus why list`

Every operational moment, narrowed by any facet the catalogue reports

```text
majordomus why list [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--audience` | `<AUDIENCE>` | — | Only moments this audience recognises (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only moments in this operational area (accepted by every subcommand) |
| `--tag` | `<TAG>` | — | Only moments carrying this tag (accepted by every subcommand) |
| `--severity` | `<SEVERITY>` | — | Only moments of this severity (accepted by every subcommand) |
| `--frequency` | `<FREQUENCY>` | — | Only moments of this frequency (accepted by every subcommand) |
| `--lifecycle` | `<LIFECYCLE>` | — | Only moments at this stage of work (accepted by every subcommand) |
| `--capability` | `<CAPABILITY>` | — | Only moments naming this capability of the executable (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only moments naming this command (accepted by every subcommand) |
| `--featured` | flag | — | Only the moments the homepage features (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated moments, not only the public ones (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies (accepted by every subcommand) |

Examples:

- **Every public moment, in presentation order** — Drafts are excluded unless `--all` is given. The facets a listing may be narrowed by are the ones the catalogue itself reports, so an audience or an area added as a file is a filter without anything being registered.

  ```console
  $ majordomus why list
  ```

  Verified: exits 0; prints SLUG.

- **Only what one audience recognises** — Membership is declared by each moment and never listed in the audience's own file, so this answer is derived. An audience the catalogue does not have is an invalid input naming the ones it does, not an empty answer.

  ```console
  $ majordomus why list --audience fixture-team
  ```

  Verified: exits 0; prints SLUG.

- **The same, as the shape the API and MCP answer with** — One domain model behind every projection: this document is what `GET /api/v1/why` returns and what the `majordomus_why` tool answers, including the derived facets and the catalogue's fingerprint.

  ```console
  $ majordomus why list --format json
  ```

  Verified: exits 0; prints one JSON document carrying /counts/moments, /facets/audiences, /fingerprint.

<a id="majordomus-why-show"></a>
## `majordomus why show`

One moment in full, with every relation derived from its metadata

```text
majordomus why show [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The moment's id, which is also its slug and its route |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--audience` | `<AUDIENCE>` | — | Only moments this audience recognises (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only moments in this operational area (accepted by every subcommand) |
| `--tag` | `<TAG>` | — | Only moments carrying this tag (accepted by every subcommand) |
| `--severity` | `<SEVERITY>` | — | Only moments of this severity (accepted by every subcommand) |
| `--frequency` | `<FREQUENCY>` | — | Only moments of this frequency (accepted by every subcommand) |
| `--lifecycle` | `<LIFECYCLE>` | — | Only moments at this stage of work (accepted by every subcommand) |
| `--capability` | `<CAPABILITY>` | — | Only moments naming this capability of the executable (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only moments naming this command (accepted by every subcommand) |
| `--featured` | flag | — | Only the moments the homepage features (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated moments, not only the public ones (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies (accepted by every subcommand) |

Examples:

- **One moment, with every relation derived from its metadata** — The record as its file declares it, then what nobody authored: the responsibilities its claims belong to, the moments that name it, and the moments nearest it by shared area, audience and tag.

  ```console
  $ majordomus why show fixture-moment
  ```

  Verified: exits 0; prints fixture-moment, derived.

<a id="majordomus-why-audiences"></a>
## `majordomus why audiences`

Every audience, with the moments that name it

```text
majordomus why audiences [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--audience` | `<AUDIENCE>` | — | Only moments this audience recognises (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only moments in this operational area (accepted by every subcommand) |
| `--tag` | `<TAG>` | — | Only moments carrying this tag (accepted by every subcommand) |
| `--severity` | `<SEVERITY>` | — | Only moments of this severity (accepted by every subcommand) |
| `--frequency` | `<FREQUENCY>` | — | Only moments of this frequency (accepted by every subcommand) |
| `--lifecycle` | `<LIFECYCLE>` | — | Only moments at this stage of work (accepted by every subcommand) |
| `--capability` | `<CAPABILITY>` | — | Only moments naming this capability of the executable (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only moments naming this command (accepted by every subcommand) |
| `--featured` | flag | — | Only the moments the homepage features (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated moments, not only the public ones (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies (accepted by every subcommand) |

Examples:

- **Who recognises what, with the counts derived** — Each audience with how many public moments name it. The number is computed from the moments; an audience's own file never lists one.

  ```console
  $ majordomus why audiences
  ```

  Verified: exits 0; prints SLUG, TITLE.

<a id="majordomus-why-areas"></a>
## `majordomus why areas`

Every operational area, with the moments that fall under it

```text
majordomus why areas [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--audience` | `<AUDIENCE>` | — | Only moments this audience recognises (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only moments in this operational area (accepted by every subcommand) |
| `--tag` | `<TAG>` | — | Only moments carrying this tag (accepted by every subcommand) |
| `--severity` | `<SEVERITY>` | — | Only moments of this severity (accepted by every subcommand) |
| `--frequency` | `<FREQUENCY>` | — | Only moments of this frequency (accepted by every subcommand) |
| `--lifecycle` | `<LIFECYCLE>` | — | Only moments at this stage of work (accepted by every subcommand) |
| `--capability` | `<CAPABILITY>` | — | Only moments naming this capability of the executable (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only moments naming this command (accepted by every subcommand) |
| `--featured` | flag | — | Only the moments the homepage features (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated moments, not only the public ones (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies (accepted by every subcommand) |

Examples:

- **The operational areas, with the counts derived** — The same relation read the other way: each area with the public moments that fall under it.

  ```console
  $ majordomus why areas
  ```

  Verified: exits 0; prints SLUG, TITLE.

<a id="majordomus-why-diagnose"></a>
## `majordomus why diagnose`

What the symptoms you recognise imply: the areas they weigh towards and the mechanisms that answer them

```text
majordomus why diagnose [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--signal` | `<SIGNALS>` | — | A signal id or a moment id; repeat for each one you recognise. Without any, the questionnaire is printed |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--audience` | `<AUDIENCE>` | — | Only moments this audience recognises (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only moments in this operational area (accepted by every subcommand) |
| `--tag` | `<TAG>` | — | Only moments carrying this tag (accepted by every subcommand) |
| `--severity` | `<SEVERITY>` | — | Only moments of this severity (accepted by every subcommand) |
| `--frequency` | `<FREQUENCY>` | — | Only moments of this frequency (accepted by every subcommand) |
| `--lifecycle` | `<LIFECYCLE>` | — | Only moments at this stage of work (accepted by every subcommand) |
| `--capability` | `<CAPABILITY>` | — | Only moments naming this capability of the executable (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only moments naming this command (accepted by every subcommand) |
| `--featured` | flag | — | Only the moments the homepage features (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated moments, not only the public ones (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies (accepted by every subcommand) |

Examples:

- **The questionnaire, assembled from the catalogue's own signals** — With no selection there is nothing to diagnose, so the questions are printed instead of an empty answer. Every line is a signal a moment declares; nothing here is a list of questions.

  ```console
  $ majordomus why diagnose
  ```

  Verified: exits 0; prints Which of these happened to you this week?.

- **What the symptoms you recognise imply** — A name is a signal id or a moment id. The answer is counting, not inference: each recommendation carries the moments that produced it, and there is no percentage because there is no model behind one.

  ```console
  $ majordomus why diagnose --signal fixture-signal
  ```

  Verified: exits 0; prints moment(s) matched, fixture-moment.

<a id="majordomus-why-validate"></a>
## `majordomus why validate`

Every finding over the catalogue; exit 10 when any is an error

```text
majordomus why validate [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--audience` | `<AUDIENCE>` | — | Only moments this audience recognises (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only moments in this operational area (accepted by every subcommand) |
| `--tag` | `<TAG>` | — | Only moments carrying this tag (accepted by every subcommand) |
| `--severity` | `<SEVERITY>` | — | Only moments of this severity (accepted by every subcommand) |
| `--frequency` | `<FREQUENCY>` | — | Only moments of this frequency (accepted by every subcommand) |
| `--lifecycle` | `<LIFECYCLE>` | — | Only moments at this stage of work (accepted by every subcommand) |
| `--capability` | `<CAPABILITY>` | — | Only moments naming this capability of the executable (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only moments naming this command (accepted by every subcommand) |
| `--featured` | flag | — | Only the moments the homepage features (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated moments, not only the public ones (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies (accepted by every subcommand) |

Examples:

- **Check the catalogue before anything projects it** — A reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; a public record that does not meet the floor its status promises. Exit 10 on any error.

  ```console
  $ majordomus why validate
  ```

  Verified: exits 0; prints moment(s), valid.

<a id="majordomus-distribution"></a>
## `majordomus distribution`

How this project is packaged, published and installed: the platforms, the artifact names, the installer, the releases

Subcommands: [`majordomus distribution show`](#majordomus-distribution-show), [`majordomus distribution status`](#majordomus-distribution-status), [`majordomus distribution validate`](#majordomus-distribution-validate), [`majordomus distribution targets`](#majordomus-distribution-targets), [`majordomus distribution matrix`](#majordomus-distribution-matrix), [`majordomus distribution artifact`](#majordomus-distribution-artifact), [`majordomus distribution releases`](#majordomus-distribution-releases), [`majordomus distribution metadata`](#majordomus-distribution-metadata), [`majordomus distribution build`](#majordomus-distribution-build).

```text
majordomus distribution [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **How this project is installed** — `distribution` with nothing after it shows the model: the one-line install command, where an installation goes, and how many platforms a release builds. Every value comes from share/distribution.yaml, which is the only place any of them is written.

  ```console
  $ majordomus distribution
  ```

  Verified: exits 0; prints binary, install, targets.

<a id="majordomus-distribution-show"></a>
## `majordomus distribution show`

The model: the install command, where an installation goes, and every declared target

```text
majordomus distribution show [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The distribution model as one JSON document** — The same answer as a document a script can read: the install command, the default locations, and every declared target with the artifact name the naming function derives for it. This is what the website's install block and the cockpit's install card render.

  ```console
  $ majordomus distribution show --format json
  ```

  Verified: exits 0; prints one JSON document carrying /install_command, /targets.

<a id="majordomus-distribution-status"></a>
## `majordomus distribution status`

Whether the advertised one-line installation works right now, and what is missing when it does not

```text
majordomus distribution status [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Whether the published one-line installation works right now** — The operator's question — *can a machine that has never seen this project install it with the advertised command?* — answered from the distribution model and the release records, without touching the network. Each check names what was observed; a failing one names its cause and the command that changes it. Shown here in a repository that has published nothing, where the answer is no and the exit code is 10, which is what makes it usable as a check rather than as prose. `distribution validate` is the gate over the model itself; this is the gate over the state a user meets.

  ```console
  $ majordomus distribution status
  ```

  Verified: exits 10.

<a id="majordomus-distribution-validate"></a>
## `majordomus distribution validate`

Every invariant of the model and of the release records; exit 10 with each violation named

```text
majordomus distribution validate [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Every invariant of the model and of the release records** — Refuses a duplicate target id or triple, two targets deriving one artifact name, a Linux target with no C library, a published target with nothing to build it on, an unbuilt target with no recorded reason, a base URL that is not HTTPS, and a release record that misses a supported target, renames an artifact or serves one from another host. Exits 10 with each violation named.

  ```console
  $ majordomus distribution validate
  ```

  Verified: exits 0; prints distribution.

<a id="majordomus-distribution-targets"></a>
## `majordomus distribution targets`

Every declared target, one line each, with the artifact name it derives

```text
majordomus distribution targets [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Every platform, and whether a release builds it** — One line per declared target: its id, its Rust target triple, whether it is supported, experimental or unavailable, and how it is written in prose. The supported-platform table in the documentation and the installer's own refusal message are rendered from these same rows.

  ```console
  $ majordomus distribution targets
  ```

  Verified: exits 0; prints RUST TARGET, supported.

<a id="majordomus-distribution-matrix"></a>
## `majordomus distribution matrix`

The release build matrix, as the release workflow reads it

```text
majordomus distribution matrix [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The release build matrix the workflow runs** — One entry per published target, with the runner it is built on, the packages that runner needs, and the artifact name with `{tag}` where a release's tag goes. The release workflow reads this and states no platform of its own; adding a target to the model adds a build here and nowhere else.

  ```console
  $ majordomus distribution matrix
  ```

  Verified: exits 0; prints one JSON document carrying /include, /binary.

<a id="majordomus-distribution-artifact"></a>
## `majordomus distribution artifact`

The archive name and root directory a target and a tag derive

```text
majordomus distribution artifact [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--target` | `<TARGET>` | required | A target's id or its Rust target triple |
| `--tag` | `<TAG>` | required | The tag, `v` and a version |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What one target and one tag are called** — The one naming function, asked directly: the archive's name, the directory it unpacks into, and where a release publishes it. `scripts/release-package` asks this rather than composing a name, so a change to the naming function reaches the packaging without an edit.

  ```console
  $ majordomus distribution artifact --target aarch64-apple-darwin --tag v0.2.0 --format json
  ```

  Verified: exits 0; prints one JSON document carrying /name, /root, /url.

<a id="majordomus-distribution-releases"></a>
## `majordomus distribution releases`

Every recorded release, newest first, and the one an unpinned installation resolves to

```text
majordomus distribution releases [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What has been published, and what an unpinned install resolves to** — Every release record this repository holds, newest first, and which of them the stable pointer names: the highest version among the stable, unwithdrawn records. The pointer is derived on every read and is authored nowhere.

  ```console
  $ majordomus distribution releases
  ```

  Verified: exits 0.

<a id="majordomus-distribution-metadata"></a>
## `majordomus distribution metadata`

The public metadata one release record publishes, rendered from the record alone

```text
majordomus distribution metadata [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--record` | `<FILE>` | required | A release record; the file the release pipeline writes under .ai/repo/releases/ |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What one release record publishes** — The public metadata a record turns into, rendered from the record and the model alone: the release pipeline prints this before it commits anything, and the installer's own tests serve it as a release that never existed. Shown here in a repository that has published nothing, where the record does not exist and the command says which file it wanted and exits 10 rather than inventing one.

  ```console
  $ majordomus distribution metadata --record .ai/repo/releases/v0.2.0.yaml
  ```

  Verified: exits 10.

<a id="majordomus-distribution-build"></a>
## `majordomus distribution build`

What this executable is: version, target triple, profile, commit

```text
majordomus distribution build [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What this executable is** — The crate version, the Rust target triple, the profile and the commit, all compiled in at build time. An installed binary answers this without a repository, a toolchain or git, which is what makes a support question answerable.

  ```console
  $ majordomus distribution build
  ```

  Verified: exits 0; prints version, target, commit.

<a id="majordomus-env"></a>
## `majordomus env`

What this checkout is: the project, version control, the toolchains it declares, what the layer holds, the workflows, the provider projections and the local services

Subcommands: [`majordomus env status`](#majordomus-env-status), [`majordomus env banner`](#majordomus-env-banner), [`majordomus env export`](#majordomus-env-export), [`majordomus env explain`](#majordomus-env-explain).

```text
majordomus env [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What this checkout is** — `env` with nothing after it resolves the whole snapshot: the project and its version, the repository and its layer, version control, the toolchains the repository declares, what the layer holds counted per kind, the workflows the runner describes, the provider projections against the policy that renders them, and the local services. This is the resolution that counts the layer, so it builds the index and writes the cache the banner reads.

  ```console
  $ majordomus env
  ```

  Verified: exits 0; prints project, repository, resolution.

<a id="majordomus-env-status"></a>
## `majordomus env status`

The whole snapshot, resolved in full: what the layer holds is counted, and the cache the banner reads is written

```text
majordomus env status [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The snapshot as one document** — The same value the HTTP route `/api/v1/environment` and the MCP resource `majordomus://environment` answer with, and the value the banner renders. Every field carries where it came from under `provenance`, and a value nothing could resolve is absent rather than zero.

  ```console
  $ majordomus env status --format json
  ```

  Verified: exits 0; prints one JSON document carrying /schema, /project/version, /repository/name, /provenance.

<a id="majordomus-env-banner"></a>
## `majordomus env banner`

Render the snapshot for a terminal. Goes to standard error, never standard output, because direnv reads standard output as the environment it is setting

```text
majordomus env banner [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--mode` | `<MODE>` | — | How much to show: `auto`, `full`, `compact` or `off`. Without it, MAJORDOMUS_BANNER decides, and without that, `auto` — which is silent when nothing is watching, shows the whole box when the repository has something new to say, and the two-line form when it does not |
| `--width` | `<COLUMNS>` | — | Draw as if the terminal were this wide, whatever it is |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The two-line form, at a width you choose** — What `direnv` renders on entering the repository. It resolves fast — it never builds the index — and it writes to standard error, because direnv reads the standard output of a `.envrc` as the environment it is applying. `--width` renders as if the terminal were that wide, which is what makes the layout testable.

  ```console
  $ majordomus env banner --mode compact --width 80
  ```

  Verified: exits 0.

<a id="majordomus-env-export"></a>
## `majordomus env export`

The variable assignments a shell in this repository benefits from, for `eval`. Assignments only: no command, no side effect

```text
majordomus env export [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--shell` | `<SHELL>` | `direnv` | The shell to write for: `direnv`, `bash`, `zsh`, `sh`, `ksh` or `fish` |
| `--banner` | flag | — | Also draw the banner, to standard error, from the same snapshot. What an adapter asks for: one process on the path a shell takes on every entry, rather than two that each pay for a `git status` |
| `--mode` | `<MODE>` | — | With --banner, how much to show; MAJORDOMUS_BANNER decides without it |
| `--bridge` | flag | — | Also refresh the workflow bridge under .ai/local/cache/ when a declaration behind it has changed. A few `stat` calls when nothing has; never a build, never a network call |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The assignments a shell in this repository wants** — Assignments and nothing else, safe to `eval`: no command runs, no file is touched, and every value is quoted so that a repository path holding a quote or a `$(...)` cannot become shell code. This is the whole of what `.envrc` needs from Majordomus.

  ```console
  $ majordomus env export --shell direnv
  ```

  Verified: exits 0; prints export MAJORDOMUS_ROOT=.

<a id="majordomus-env-explain"></a>
## `majordomus env explain`

Where each value came from: the file, command or constant that decided it, the resolver that read it, and how far it can be trusted

```text
majordomus env explain [OPTIONS] [FIELD]
```

| argument | value | default | description |
|---|---|---|---|
| `<FIELD>` | `<FIELD>` | — | One field in dotted form (`vcs.branch`, `layer.objects`), or a prefix; every field when absent |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Where one value came from** — An inferred system without provenance is magic. Every field of the snapshot can name the file, command or compile-time constant that decided it, the resolver that read it, and whether it was read now, taken from the cache, or not resolved at all.

  ```console
  $ majordomus env explain project.version
  ```

  Verified: exits 0; prints project.version, source, resolver.

<a id="majordomus-commands"></a>
## `majordomus commands`

Every command this repository offers, from whichever program offers it: the graph, one command, where each one is projected, and the workflow bridge derived from it

Subcommands: [`majordomus commands list`](#majordomus-commands-list), [`majordomus commands show`](#majordomus-commands-show), [`majordomus commands explain`](#majordomus-commands-explain), [`majordomus commands graph`](#majordomus-commands-graph), [`majordomus commands bridge`](#majordomus-commands-bridge).

```text
majordomus commands [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Every command this repository offers** — The command graph, composed from the three declarations that already exist: the clap tree of this executable, the shipped command registry of the shell tool, and the recipes the workflow runner describes. One line per command, with the program that runs it and what running it changes.

  ```console
  $ majordomus commands
  ```

  Verified: exits 0; prints commands, executable, read-only.

<a id="majordomus-commands-list"></a>
## `majordomus commands list`

Every command, one line each: what it is, what running it changes, and where it is projected

```text
majordomus commands list [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--origin` | `executable` \| `tool` \| `workflow` | — | Only the commands of this program — `executable`: This executable; `tool`: The shell tool, bin/majordomus; `workflow`: A workflow the repository declares |
| `--effect` | `read-only` \| `local-mutation` \| `repository-mutation` \| `network-mutation` \| `destructive` | — | Only the commands whose effect is at most this — `read-only`: Reads and answers; `local-mutation`: Writes only what no commit carries; `repository-mutation`: Writes tracked files; `network-mutation`: Reaches the network with an effect; `destructive`: Removes something |
| `--search` | `<TEXT>` | — | Only the commands matching this text, in their invocation, summary, tags or identity |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Only what reads** — The filters are the graph's own vocabulary rather than a search over text: `--effect read-only` is every command that changes nothing anywhere, which is the same predicate the exposure policy uses to decide what a machine surface may call.

  ```console
  $ majordomus commands list --effect read-only
  ```

  Verified: exits 0; prints read-only.

<a id="majordomus-commands-show"></a>
## `majordomus commands show`

One command in full: its arguments, its effect, what it needs, and every surface that carries it

```text
majordomus commands show [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The command's identity, `executable.worktree.status` |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **One command, and every surface that carries it** — The arguments with the source of each one's values, the effect, and the projections: the command line, the workflow recipe, the MCP tool, the HTTP route, the Cockpit and the page. A surface that withholds it says why.

  ```console
  $ majordomus commands show executable.worktree.status
  ```

  Verified: exits 0; prints executable.worktree.status, projections.

<a id="majordomus-commands-explain"></a>
## `majordomus commands explain`

Why one command appears where it does: the declaration it came from, the policy that placed it, and the reason for every surface that withholds it

```text
majordomus commands explain [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The command's identity, `executable.worktree.status` |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Why a command appears where it does** — The same command with its provenance: the file that declares it, the reader that found it, the capability behind it when there is one, what it requires, and the file the exposure policy lives in. Nothing about a command's placement is a mystery a grep has to solve.

  ```console
  $ majordomus commands explain executable.serve
  ```

  Verified: exits 0; prints declared in, policy.

<a id="majordomus-commands-graph"></a>
## `majordomus commands graph`

The whole graph as one document, with its fingerprint and every diagnostic

```text
majordomus commands graph [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--check` | flag | — | Exit 10 when the graph carries an error |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The whole graph as one document** — Deterministic and fingerprinted: two builds over one tree produce the same bytes, which is what lets the workflow bridge, the completion index and the Cockpit all key on the fingerprint instead of regenerating.

  ```console
  $ majordomus commands graph --format json
  ```

  Verified: exits 0; prints one JSON document carrying /schema, /fingerprint, /commands.

<a id="majordomus-commands-bridge"></a>
## `majordomus commands bridge`

Materialise the workflow bridge from the graph, and refresh the cache the completion reads; writes nothing when the graph has not changed

```text
majordomus commands bridge [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--check` | flag | — | Exit 10 when the materialised bridge is not the one this graph projects; write nothing |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The workflow runner's recipes, derived** — Every command of both programs, written as a recipe that runs the canonical program with the caller's own arguments. It goes under .ai/local/cache/, which no commit carries, and it is rewritten only when the graph's fingerprint changes.

  ```console
  $ majordomus commands bridge
  ```

  Verified: exits 0; prints bridge, recipe.

<a id="majordomus-completion"></a>
## `majordomus completion`

Completion for any surface, answered from the command graph: the candidates a shell asks for, and the one-time integration that asks

Subcommands: [`majordomus completion query`](#majordomus-completion-query), [`majordomus completion init`](#majordomus-completion-init), [`majordomus completion install`](#majordomus-completion-install).

```text
majordomus completion [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **The integration a person installs once** — With no subcommand, the shell integration for zsh. It contains no command, no flag and no identifier: every candidate comes from a query against the command graph of the repository the shell is in, so one integration serves every checkout and never goes stale.

  ```console
  $ majordomus completion
  ```

  Verified: exits 0; prints completion query, compdef.

<a id="majordomus-completion-query"></a>
## `majordomus completion query`

The candidates for one command line, from the command graph. What a shell adapter calls on every TAB

```text
majordomus completion query [OPTIONS] [WORD]
```

| argument | value | default | description |
|---|---|---|---|
| `--surface` | `cli` \| `workflow` | `cli` | Which surface the words are spelled for — `cli`: The command line of either program; `workflow`: The workflow runner |
| `--cursor` | `<N>` | — | The index of the word the cursor is in; the default is a new word after the last |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `<WORD>` | `<WORD>` | — | The words of the command line, the program's own name first |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **What a shell asks on every TAB** — The words of the command line and the position of the cursor; back come the candidates with their descriptions. The same call answers the workflow runner's completion with `--surface workflow`, resolving the recipe name to the command it bridges and then completing that command's own arguments.

  ```console
  $ majordomus completion query --surface cli -- majordomus work
  ```

  Verified: exits 0; prints worktree.

<a id="majordomus-completion-init"></a>
## `majordomus completion init`

The shell integration to load once, which carries no command of its own and asks this executable for every candidate

```text
majordomus completion init [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--shell` | `zsh` \| `bash` \| `fish` | `zsh` | Which shell to print the integration for — `zsh`: zsh; `bash`: bash; `fish`: fish |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **The same, for bash** — A different shell's protocol, the same question. Both adapters read the words being completed, find the cursor, ask this executable and print what comes back.

  ```console
  $ majordomus completion init --shell bash
  ```

  Verified: exits 0; prints completion query, complete -F.

<a id="majordomus-completion-install"></a>
## `majordomus completion install`

Put that integration into the shell's startup file, between managed markers, so that no one maintains it by hand

```text
majordomus completion install [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--shell` | `zsh` \| `bash` \| `fish` | `zsh` | Which shell to install for; decides the startup file when --rc is not given — `zsh`: zsh; `bash`: bash; `fish`: fish |
| `--rc` | `<PATH>` | — | The startup file to write, instead of the shell's usual one |
| `--remove` | flag | — | Take the block out again, leaving the rest of the file as it was |
| `--dry-run` | flag | — | Say what would change and write nothing |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |

Examples:

- **The one line a person adds to their shell, added for them** — Writes the integration into the shell's startup file between `# >>> MAJORDOMUS >>>` markers: nothing outside them is touched, running it twice changes nothing, and `--remove` takes it out again. It is never a side effect of anything else — installing into a person's home directory is its own decision, so it is its own command. `--dry-run` says what would change and writes nothing.

  ```console
  $ majordomus completion install --shell zsh --dry-run
  ```

  Verified: exits 0.

<a id="majordomus-worktree"></a>
## `majordomus worktree`

The branch-to-worktree topology: where every linked worktree belongs (`<repo>-wt/<branch>`), where each one is, and the lifecycle — create, migrate, repair, guard

Subcommands: [`majordomus worktree status`](#majordomus-worktree-status), [`majordomus worktree list`](#majordomus-worktree-list), [`majordomus worktree topology`](#majordomus-worktree-topology), [`majordomus worktree root`](#majordomus-worktree-root), [`majordomus worktree path`](#majordomus-worktree-path), [`majordomus worktree inspect`](#majordomus-worktree-inspect), [`majordomus worktree create`](#majordomus-worktree-create), [`majordomus worktree ensure`](#majordomus-worktree-ensure), [`majordomus worktree migrate`](#majordomus-worktree-migrate), [`majordomus worktree validate`](#majordomus-worktree-validate), [`majordomus worktree doctor`](#majordomus-worktree-doctor), [`majordomus worktree guard`](#majordomus-worktree-guard), [`majordomus worktree repair`](#majordomus-worktree-repair), [`majordomus worktree remove`](#majordomus-worktree-remove), [`majordomus worktree cleanup`](#majordomus-worktree-cleanup), [`majordomus worktree branches`](#majordomus-worktree-branches).

```text
majordomus worktree [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Where am I, and is that where I belong?** — `worktree` with nothing after it answers the question a worker asks before starting: which branch this is, whether this directory is that branch's canonical worktree (or the primary checkout on the trunk), what is uncommitted here, and how many errors the whole topology carries. The same answer from the primary checkout and from four directories deep inside a linked worktree.

  ```console
  $ majordomus worktree
  ```

  Verified: exits 0; prints branch, worktree, container.

<a id="majordomus-worktree-status"></a>
## `majordomus worktree status`

Where this call is — branch, worktree, canonical or not, uncommitted work — and how many errors the whole topology carries; exit 10 when this worktree is out of place

```text
majordomus worktree status [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The current worktree as one document** — The same answer as JSON: the repository, the container, the trunk and how it was decided, this worktree with its standing and diagnostics, and whether it is where it belongs. This is what the MCP tool `majordomus_worktree_status` and `GET /api/v1/worktrees/status` answer.

  ```console
  $ majordomus worktree status --format json
  ```

  Verified: exits 0; prints one JSON document carrying /worktree/standing, /container/path, /trunk/source, /canonical.

<a id="majordomus-worktree-list"></a>
## `majordomus worktree list`

Every registered worktree with its standing, one line each; exit 10 when the topology has an error

```text
majordomus worktree list [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Every worktree, the misplaced ones obvious** — The primary checkout first, then every linked worktree with its standing, its branch, where it belongs when it is somewhere else, and its uncommitted work. The primary checkout is exempt from the path rule and held to the trunk rule instead.

  ```console
  $ majordomus worktree create feature/example
  $ majordomus worktree list
  ```

  Verified: exits 0; prints PRIMARY, CANONICAL, -wt/feature/example.

<a id="majordomus-worktree-topology"></a>
## `majordomus worktree topology`

The whole topology: repository, container, trunk, every worktree, every branch without a worktree, every diagnostic; exit 10 when it has an error

```text
majordomus worktree topology [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The whole topology as one document** — The repository, the container, the trunk, every worktree, every branch with or without a worktree, every diagnostic with its code and remedy, and the tallies. This is what the MCP resource `majordomus://worktrees`, the tool `majordomus_worktrees`, `GET /api/v1/worktrees` and the Cockpit all render.

  ```console
  $ majordomus worktree topology --format json
  ```

  Verified: exits 0; prints one JSON document carrying /container/path, /trunk/branch, /worktrees/0/standing, /branches/0/name, /tallies/worktrees, /valid.

<a id="majordomus-worktree-root"></a>
## `majordomus worktree root`

Print the container every linked worktree belongs under, and nothing else: `cd "$(majordomus worktree root)"`

```text
majordomus worktree root [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The container, for the shell** — Prints the container and nothing else, so a shell can use it: `cd "$(majordomus worktree root)"`. It is the primary checkout's sibling named with `-wt`, derived from git's own identity and never from the current directory — which is why running this inside a linked worktree does not answer a container inside that worktree.

  ```console
  $ majordomus worktree root
  ```

  Verified: exits 0; prints -wt.

<a id="majordomus-worktree-path"></a>
## `majordomus worktree path`

Print the canonical path of a branch, and nothing else: `cd "$(majordomus worktree path feature/x)"`. Derived from the name; the branch need not exist

```text
majordomus worktree path [OPTIONS] <BRANCH>
```

| argument | value | default | description |
|---|---|---|---|
| `<BRANCH>` | `<BRANCH>` | required | The branch, full name |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The canonical path of a branch, for the shell** — A child process cannot change its parent shell's directory, so nothing here pretends to: this prints one path and the shell does the rest — `cd "$(majordomus worktree path feature/x)"`. The path is the branch name under the container, hierarchy kept; the branch need not exist yet.

  ```console
  $ majordomus worktree path feature/providers/streaming
  ```

  Verified: exits 0; prints -wt/feature/providers/streaming.

<a id="majordomus-worktree-inspect"></a>
## `majordomus worktree inspect`

One branch: its canonical path, whether it exists, what occupies the path, the worktree holding it, and what stands in the way

```text
majordomus worktree inspect [OPTIONS] <BRANCH>
```

| argument | value | default | description |
|---|---|---|---|
| `<BRANCH>` | `<BRANCH>` | required | The branch, full name |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **One branch, before creating its worktree** — Where the branch's worktree belongs, whether the branch exists, whether anything occupies the path, and what would stand in the way. For a branch that does not exist yet, the answer is the path `worktree create` would use and the command to run.

  ```console
  $ majordomus worktree inspect feature/new-dashboard
  ```

  Verified: exits 0; prints -wt/feature/new-dashboard, does not exist yet.

<a id="majordomus-worktree-create"></a>
## `majordomus worktree create`

Create the canonical worktree of a branch, creating the branch from --base (default: the trunk) when it does not exist. The path is derived; none may be given

```text
majordomus worktree create [OPTIONS] [BRANCH]
```

| argument | value | default | description |
|---|---|---|---|
| `<BRANCH>` | `<BRANCH>` | — | The branch, full name (`feature/improve-cli`) |
| `--base` | `<REF>` | — | Start a new branch from this ref. Never fetched: it must resolve locally |
| `--issue` | `<ID>` | — | Name the branch after this issue of .ai/repo/project/issues: `feature/<id>-<slug>`, the form the topology reads the issue back from |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Start work on a branch without deciding where it goes** — Creates the branch `feature/improve-cli` from the trunk and checks it out in a new worktree at `<repository>-wt/feature/improve-cli`. No path is given and none may be: the destination follows from the repository's identity and the branch name, so the same command in the same repository always produces the same path — from the primary checkout, and from inside another worktree.

  ```console
  $ majordomus worktree create feature/improve-cli
  ```

  Verified: exits 0; prints -wt/feature/improve-cli, feature/improve-cli (new.

<a id="majordomus-worktree-ensure"></a>
## `majordomus worktree ensure`

The canonical worktree of a branch: created when absent, answered when present, refused when the branch is checked out somewhere else

```text
majordomus worktree ensure [OPTIONS] <BRANCH>
```

| argument | value | default | description |
|---|---|---|---|
| `<BRANCH>` | `<BRANCH>` | required | The branch, full name |
| `--base` | `<REF>` | — | Start a new branch from this ref (default: the trunk) |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The canonical worktree, whether or not it exists yet** — `ensure` is `create` for a caller that does not care whether the worktree is already there: it creates it when it is absent and answers the existing one when it is present. What a script or an agent runs before starting on a branch.

  ```console
  $ majordomus worktree create feature/improve-cli
  $ majordomus worktree ensure feature/improve-cli
  ```

  Verified: exits 0; prints exists, -wt/feature/improve-cli.

<a id="majordomus-worktree-migrate"></a>
## `majordomus worktree migrate`

Bring every misplaced worktree to its canonical path, dirty state included, with a fingerprint taken before and after each move; --plan shows the steps and changes nothing

```text
majordomus worktree migrate [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--plan` | flag | — | Show the plan and change nothing |
| `--dry-run` | flag | — | The same as --plan |
| `--allow-copy` | flag | — | When a move crosses filesystems, copy the tree, repair git's link, verify the copy against a manifest of every entry, and only then remove the original |
| `--only` | `<BRANCH>` | — | Only these branches |
| `--include-ephemeral` | flag | — | Also move the scratch checkouts of sessions (under the temporary directory or .claude/worktrees), which are otherwise reported and left alone |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What it would take to bring every worktree home** — `--plan` shows each misplaced worktree with where it belongs, how it would move, the uncommitted work that moves with it, and what blocks it, and changes nothing. Without `--plan` the movable steps are carried out: each worktree is fingerprinted, moved with `git worktree move`, fingerprinted again at its new path, and reported as moved only when the two are equal.

  ```console
  $ majordomus worktree migrate --plan
  ```

  Verified: exits 0; prints nothing to migrate.

<a id="majordomus-worktree-validate"></a>
## `majordomus worktree validate`

Every error of the topology, and nothing else; exit 10 when there is one

```text
majordomus worktree validate [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Is the topology valid?** — Every error-level diagnostic and nothing else, then the verdict; exit 10 when there is an error. What a script gates on.

  ```console
  $ majordomus worktree validate
  ```

  Verified: exits 0; prints worktree topology: valid.

<a id="majordomus-worktree-doctor"></a>
## `majordomus worktree doctor`

Every diagnostic of the topology, errors, warnings and facts, each with its code and remedy; exit 10 when there is an error

```text
majordomus worktree doctor [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Every diagnostic, with its code and its remedy** — Errors, warnings and facts — a misplaced worktree, a stale registration, a detached HEAD, the primary checkout off the trunk, an unknown trunk — each under a stable code the API and the Cockpit carry too, each with the command that addresses it.

  ```console
  $ majordomus worktree doctor
  ```

  Verified: exits 0; prints worktree topology: valid.

<a id="majordomus-worktree-guard"></a>
## `majordomus worktree guard`

May a mutation proceed from here? Exit 0 in a canonical worktree, in the primary checkout on the trunk, or detached; exit 10 with the reason otherwise. What the pre-commit hook asks

```text
majordomus worktree guard [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `-q`, `--quiet` | flag | — | Print nothing on success |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **May a commit proceed from here?** — The pre-commit hook's question. Exit 0 in a branch's canonical worktree, in the primary checkout on the trunk, or on a detached HEAD; exit 10 with the diagnostic and the remedy when a feature branch is being worked on somewhere it does not belong. The hook stays one line; this is the logic.

  ```console
  $ majordomus worktree guard
  ```

  Verified: exits 0; prints worktree guard: ok.

<a id="majordomus-worktree-repair"></a>
## `majordomus worktree repair`

Drop git's registrations of worktrees whose directories are gone, and repair the administrative links of the ones that exist. Deletes no directory

```text
majordomus worktree repair [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--dry-run` | flag | — | Report what would be dropped and change nothing |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What git would forget** — `repair` drops git's registrations of worktrees whose directories no longer exist and lets git repair the administrative links of the ones that do. It deletes no directory and touches no branch; `--dry-run` reports what it would drop and changes nothing.

  ```console
  $ majordomus worktree repair --dry-run
  ```

  Verified: exits 0; prints nothing to prune.

<a id="majordomus-worktree-remove"></a>
## `majordomus worktree remove`

Remove one linked worktree by branch or path. Never the primary checkout, never a branch, never uncommitted work without --force

```text
majordomus worktree remove [OPTIONS] <SELECTOR>
```

| argument | value | default | description |
|---|---|---|---|
| `<SELECTOR>` | `<SELECTOR>` | required | An exact branch name or an exact path |
| `--force` | flag | — | Remove it even though it holds uncommitted work or is locked |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Remove a worktree, and keep its branch** — Removes the worktree and nothing else. The branch it held still exists: worktree lifecycle and branch lifecycle are separate, and deleting a branch is a git command a person types deliberately. A worktree with uncommitted work is refused rather than removed.

  ```console
  $ majordomus worktree create feature/improve-cli
  $ majordomus worktree remove feature/improve-cli
  ```

  Verified: exits 0; prints removed, feature/improve-cli still exists.

<a id="majordomus-worktree-cleanup"></a>
## `majordomus worktree cleanup`

The branches merged into the trunk whose worktree is clean or absent: what could be removed. Removes nothing

```text
majordomus worktree cleanup [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What could go, and what it would take** — Every branch merged into the trunk whose worktree is clean or absent, with the two commands that would remove the worktree and then the branch. Derived state only: nothing is deleted here, and a dirty or unmerged worktree is never listed.

  ```console
  $ majordomus worktree cleanup
  ```

  Verified: exits 0; prints cleanup-eligible.

<a id="majordomus-worktree-branches"></a>
## `majordomus worktree branches`

Every local branch, one per line, for a shell completion that wants the live set

```text
majordomus worktree branches [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--without-worktree` | flag | — | Only branches with no worktree |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The live branch set, for completion** — Every local branch, one per line, nothing else. A shell completion for `worktree path`, `create` or `remove` reads this rather than a list kept anywhere, so a branch created a second ago completes.

  ```console
  $ majordomus worktree branches
  ```

  Verified: exits 0.

<a id="majordomus-product"></a>
## `majordomus product`

The product: what this repository's tool does for a person, as the features under the layer declare it, with every surface, count and moment derived; the matrix of features against interfaces; the providers; and the model's own validation

Subcommands: [`majordomus product list`](#majordomus-product-list), [`majordomus product show`](#majordomus-product-show), [`majordomus product matrix`](#majordomus-product-matrix), [`majordomus product providers`](#majordomus-product-providers), [`majordomus product validate`](#majordomus-product-validate).

```text
majordomus product [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--featured` | flag | — | Only the features the homepage shows (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated features, not only the stable ones (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only features serving this operational area of the why catalogue (accepted by every subcommand) |
| `--module` | `<MODULE>` | — | Only features made of this capability module (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only features made of this shell command (accepted by every subcommand) |
| `--surface` | `<SURFACE>` | — | Only features exposed through this surface: cli, api, mcp, cockpit or docs (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, headlines, summaries, tags and bodies (accepted by every subcommand) |

Examples:

- **What the product does, as the layer declares it** — `product` with nothing after it lists the features, because listing is what a person wants when they ask what the tool is for. Every column is derived: the surfaces a feature is exposed through come from the modules, commands and kinds it names, never from the file.

  ```console
  $ majordomus product
  ```

  Verified: exits 0; prints SLUG, SURFACES, feature(s).

<a id="majordomus-product-list"></a>
## `majordomus product list`

Every feature, narrowed by any filter, with the surfaces derived for each

```text
majordomus product list [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--featured` | flag | — | Only the features the homepage shows (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated features, not only the stable ones (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only features serving this operational area of the why catalogue (accepted by every subcommand) |
| `--module` | `<MODULE>` | — | Only features made of this capability module (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only features made of this shell command (accepted by every subcommand) |
| `--surface` | `<SURFACE>` | — | Only features exposed through this surface: cli, api, mcp, cockpit or docs (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, headlines, summaries, tags and bodies (accepted by every subcommand) |

Examples:

- **Every stable feature, in presentation order** — Drafts are excluded unless `--all` is given; `--featured` narrows to the features the homepage shows. The filters are the facets the model derives — an area, a module, a command, a surface — so a module added to the executable is a filter without anything being registered.

  ```console
  $ majordomus product list
  ```

  Verified: exits 0; prints SLUG, fixture-feature.

- **The same, as the shape the API and MCP answer with** — One domain model behind every projection: this document is what `GET /api/v1/product/features` returns and what the `majordomus_features` tool answers, with the counts, the fingerprint and the surfaces of every feature.

  ```console
  $ majordomus product list --format json
  ```

  Verified: exits 0; prints one JSON document carrying /counts/features, /features/0/surfaces, /fingerprint.

<a id="majordomus-product-show"></a>
## `majordomus product show`

One feature in full: what it is made of, resolved, and everything derived from that

```text
majordomus product show [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The feature's id, which is also its slug and its route |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--featured` | flag | — | Only the features the homepage shows (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated features, not only the stable ones (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only features serving this operational area of the why catalogue (accepted by every subcommand) |
| `--module` | `<MODULE>` | — | Only features made of this capability module (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only features made of this shell command (accepted by every subcommand) |
| `--surface` | `<SURFACE>` | — | Only features exposed through this surface: cli, api, mcp, cockpit or docs (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, headlines, summaries, tags and bodies (accepted by every subcommand) |

Examples:

- **One feature, with everything derived from what it names** — The record as its file declares it, then what nobody authored: the capabilities of its modules with their tools and routes, the commands with their summaries, the objects of its kinds counted, the rules with their class, the documents, the decisions, the claims with their status, the moments it answers, and the interfaces all of that adds up to.

  ```console
  $ majordomus product show fixture-feature
  ```

  Verified: exits 0; prints fixture-feature, surfaces, derived.

<a id="majordomus-product-matrix"></a>
## `majordomus product matrix`

Every feature against every interface, and every module, command and kind against the features that name it

```text
majordomus product matrix [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--featured` | flag | — | Only the features the homepage shows (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated features, not only the stable ones (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only features serving this operational area of the why catalogue (accepted by every subcommand) |
| `--module` | `<MODULE>` | — | Only features made of this capability module (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only features made of this shell command (accepted by every subcommand) |
| `--surface` | `<SURFACE>` | — | Only features exposed through this surface: cli, api, mcp, cockpit or docs (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, headlines, summaries, tags and bodies (accepted by every subcommand) |

Examples:

- **Every feature against every interface, and what no feature names** — One row per feature with a mark per surface, then every module of the executable, every public command and every kind of the layer with the features that name it. A row with no feature is a gap the product page cannot hide.

  ```console
  $ majordomus product matrix
  ```

  Verified: exits 0; prints FEATURE, cli, MODULE.

<a id="majordomus-product-providers"></a>
## `majordomus product providers`

Every provider the tool has an adapter for, with what this repository does with it

```text
majordomus product providers [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--featured` | flag | — | Only the features the homepage shows (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated features, not only the stable ones (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only features serving this operational area of the why catalogue (accepted by every subcommand) |
| `--module` | `<MODULE>` | — | Only features made of this capability module (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only features made of this shell command (accepted by every subcommand) |
| `--surface` | `<SURFACE>` | — | Only features exposed through this surface: cli, api, mcp, cockpit or docs (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, headlines, summaries, tags and bodies (accepted by every subcommand) |

Examples:

- **Every provider the tool has an adapter for** — One line per template the distribution ships, with the bootstraps this repository's policy renders through it, the client configuration it carries for the shared MCP server, and the hooks the policy wires. The set is the templates; nothing here is a list of vendors.

  ```console
  $ majordomus product providers
  ```

  Verified: exits 0; prints PROVIDER, agents.

<a id="majordomus-product-validate"></a>
## `majordomus product validate`

Every finding over the model; exit 10 when any is an error

```text
majordomus product validate [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--featured` | flag | — | Only the features the homepage shows (accepted by every subcommand) |
| `--all` | flag | — | Include drafts and deprecated features, not only the stable ones (accepted by every subcommand) |
| `--area` | `<AREA>` | — | Only features serving this operational area of the why catalogue (accepted by every subcommand) |
| `--module` | `<MODULE>` | — | Only features made of this capability module (accepted by every subcommand) |
| `--names-command` | `<NAMES_COMMAND>` | — | Only features made of this shell command (accepted by every subcommand) |
| `--surface` | `<SURFACE>` | — | Only features exposed through this surface: cli, api, mcp, cockpit or docs (accepted by every subcommand) |
| `-q`, `--query` | `<QUERY>` | — | Case-insensitive text over identities, titles, headlines, summaries, tags and bodies (accepted by every subcommand) |

Examples:

- **Check the model before anything projects it** — A reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; a draft that is featured; a stable feature under its floors; and every module, command or kind no feature names. Exit 10 on any error.

  ```console
  $ majordomus product validate
  ```

  Verified: exits 0; prints feature(s), valid.

<a id="majordomus-release"></a>
## `majordomus release`

What this project has shipped and what it would ship next: the changelog derived from the layer's own records, the version the two writers state, and the one command that raises both

Subcommands: [`majordomus release changelog`](#majordomus-release-changelog), [`majordomus release version`](#majordomus-release-version), [`majordomus release bump`](#majordomus-release-bump).

```text
majordomus release [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | How to render the answer (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What has shipped, and what has not** — `release` with nothing after it renders the changelog. Every line of it is derived — a section per release the layer records, its decisions the ADRs dated inside that release's window, its changes the conventional commits in its range — so there is no file anyone can forget to update.

  ```console
  $ majordomus release
  ```

  Verified: exits 0; prints Changelog.

<a id="majordomus-release-changelog"></a>
## `majordomus release changelog`

The changelog, composed from the layer's release records, the decisions dated inside each release's window, and the conventional commits in its range

```text
majordomus release changelog [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | How to render the answer (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The same document every other surface answers with** — What `GET /api/v1/changelog` returns, what the MCP resource `majordomus://changelog` carries, and what `majordomus generate changelog` writes into the reference. One value, four renderings.

  ```console
  $ majordomus release changelog --format json
  ```

  Verified: exits 0; prints one JSON document carrying /schema, /current, /sections.

<a id="majordomus-release-version"></a>
## `majordomus release version`

The version the two writers state, whether they agree, and the bump the commits since the last release imply

```text
majordomus release version [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | How to render the answer (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The version, and the one the commits imply** — The version is stated in two files for a reason the release script gives: an installed tree has no Cargo.toml and the crate is compiled before the shell tool exists, so neither can read the other at run time. This says what both state, whether they agree, and what the conventional commits since the last release imply the next one should be.

  ```console
  $ majordomus release version --format json
  ```

  Verified: exits 0; prints one JSON document carrying /declared, /agree, /bump.

<a id="majordomus-release-bump"></a>
## `majordomus release bump`

Raise the version in both places at once, to the bump the commits imply or to one you name

```text
majordomus release bump [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--level` | `<LEVEL>` | — | Raise by this much instead of by what the commits imply |
| `--exact` | `<VERSION>` | — | Set exactly this version, instead of raising the current one |
| `--dry-run` | flag | — | Say what would change and write nothing |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | How to render the answer (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Raising it, in both places, once** — The bump defaults to what the commits imply — a breaking change is major, a feature is minor, anything else is patch — and `--level` or `--exact` overrides that when a person means something the commits do not say. It writes both files and nothing else; `scripts/release-version --check` then proves the work of one writer rather than the memory of one person. A repository that declares no version — the example runs in one with no crate — cannot be raised, and says so with exit 12 rather than inventing a number to raise from.

  ```console
  $ majordomus release bump --dry-run
  ```

  Verified: exits 12.

<a id="majordomus-quality"></a>
## `majordomus quality`

What this executable's own public surface is held to: documentation, executable examples, module coverage, and every command accounted for against the capability registry

Subcommands: [`majordomus quality report`](#majordomus-quality-report).

```text
majordomus quality <COMMAND>
```

Arguments: none.

<a id="majordomus-quality-report"></a>
## `majordomus quality report`

Measure the crate and report every finding, with the rule it breaks and what to do about it

```text
majordomus quality report [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |
| `--code` | `<CODE>` | — | Only findings carrying this code, e.g. RUST_PUBLIC_MISSING_EXAMPLE |
| `--path` | `<PATH>` | — | Only findings under this repository-relative path prefix |
| `--summary` | flag | — | Print the counts and leave the findings out |
| `--include-baselined` | flag | — | Show the findings the baseline already accepts, which are left out by default |
| `--write-baseline` | flag | — | Record today's findings as the accepted baseline, so the debt can shrink and cannot grow |

Examples:

- **Where the crate's public surface stands** — The counts alone: how much of the exported surface is documented and exampled, how many modules something exercises, and how the canonical operations stand against the command line, HTTP, OpenAPI and MCP. Exits 10 when any finding stands outside the recorded baseline.

  ```console
  $ majordomus quality report --summary
  ```

  Verified: exits 0.

- **One kind of finding, with the rule and the remedy** — Filtered to one violation code. Every finding carries the rule that requires it, where it is, why it matters and what to do — which is what lets a person and an agent act on the same report.

  ```console
  $ majordomus quality report --code RUST_MODULE_MISSING_EXAMPLE --format json
  ```

  Verified: exits 0; prints one JSON document carrying /measured, /passes, /report/schema.

<a id="majordomus-run"></a>
## `majordomus run`

Run a capability as an execution and follow it: its steps, its progress and its output as they happen

```text
majordomus run [OPTIONS] <CAPABILITY>
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `<CAPABILITY>` | `<CAPABILITY>` | required | The capability to run, by its canonical id (`health.report`, `objects.verify`) |
| `--input` | `<JSON>` | — | Its input, as one JSON object; the capability's input schema is what validates it |
| `--follow` | flag | — | Print the events as they arrive on stderr; on by default when stderr is a terminal |
| `--quiet` | flag | — | Print nothing but the final output |
| `--format` | `text` \| `json` | `text` | Output shape — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Watch an execution happen** — `run` starts a capability as an execution and follows it to its end: every step, every line it logs and every advance of its progress, as the handler reports them. `executions.demonstrate` exists to make that visible without waiting for real work — it reads nothing and writes nothing, and its only effect is the events it produces. The same execution, started from the Cockpit, streams the same events to a browser.

  ```console
  $ majordomus run executions.demonstrate --input '{"steps":2,"delay_ms":0}' --format json
  ```

  Verified: exits 0; prints one JSON document carrying /state, /id, /output/steps, /steps/0/name.

<a id="majordomus-executions"></a>
## `majordomus executions`

The executions of the server serving this repository: what has run, what is running, and what each one said

Subcommands: [`majordomus executions list`](#majordomus-executions-list), [`majordomus executions show`](#majordomus-executions-show), [`majordomus executions events`](#majordomus-executions-events), [`majordomus executions cancel`](#majordomus-executions-cancel), [`majordomus executions protocol`](#majordomus-executions-protocol).

```text
majordomus executions [OPTIONS] [COMMAND]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **What has run** — `executions` with nothing after it lists what the server serving this repository has run, newest first. In a checkout where no server is running it says so rather than pretending: an execution lives in the process that accepted it.

  ```console
  $ majordomus executions
  ```

  Verified: exits 0; prints execution.

<a id="majordomus-executions-list"></a>
## `majordomus executions list`

Every execution the server remembers, newest first

```text
majordomus executions list [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--state` | `<STATE>` | — | Only executions in this state (queued, running, cancelling, succeeded, failed, cancelled) |
| `--capability` | `<CAPABILITY>` | — | Only executions of this capability |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Every execution, as one document** — The same answer `GET /api/v1/executions`, the MCP tool `majordomus_executions` and the Cockpit's Executions page render, with the counts beside it: how many are remembered, how many are active, how many are waiting for a worker and how many live channels are following them.

  ```console
  $ majordomus executions list --format json
  ```

  Verified: exits 0; prints one JSON document carrying /count, /active, /queued, /live_channels.

<a id="majordomus-executions-show"></a>
## `majordomus executions show`

One execution in full: its state, its steps, its diagnostics and what it produced

```text
majordomus executions show [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The execution's id |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **An execution that is not there** — An execution lives in the process that accepted it and is remembered in bounded numbers, so asking for one nothing ran says so and exits with the missing-artifact code rather than inventing an empty answer. Against a running server, the same command prints that execution's state, its steps and what it produced.

  ```console
  $ majordomus executions show x-20260101T120000Z-4c3b2a19
  ```

  Verified: exits 12.

<a id="majordomus-executions-events"></a>
## `majordomus executions events`

One execution's retained events, oldest first

```text
majordomus executions events [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The execution's id |
| `--after` | `<AFTER>` | — | Only events after this sequence number |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The events of an execution that is not there** — The retained events of one execution, oldest first, after a sequence number — what a reconnecting client reads before it opens the live channel. For an execution nothing ran, the same refusal as `show`.

  ```console
  $ majordomus executions events x-20260101T120000Z-4c3b2a19
  ```

  Verified: exits 12.

<a id="majordomus-executions-cancel"></a>
## `majordomus executions cancel`

Ask an execution to stop

```text
majordomus executions cancel [OPTIONS] <ID>
```

| argument | value | default | description |
|---|---|---|---|
| `<ID>` | `<ID>` | required | The execution's id |
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **Asking an execution that is not there to stop** — Cancellation is cooperative: the flag is set and a task stops when it next looks at it. There is nothing to set for an execution nothing ran, and the command says so rather than reporting a success it did not have.

  ```console
  $ majordomus executions cancel x-20260101T120000Z-4c3b2a19
  ```

  Verified: exits 12.

<a id="majordomus-executions-protocol"></a>
## `majordomus executions protocol`

The live channel's contract: where it is, what it writes, and the schema of each message

```text
majordomus executions protocol [OPTIONS]
```

| argument | value | default | description |
|---|---|---|---|
| `--repo` | `<PATH>` | — | Start the search for the repository root here (default: the current directory) (accepted by every subcommand) |
| `--discovery` | `vcs` \| `filesystem` | `vcs` | How declarative files are enumerated (accepted by every subcommand) — `vcs`: Tracked files, through the version-control index (the layer's contract); `filesystem`: A walk of the work tree with the same glob semantics; untracked files included |
| `--strict` | flag | — | Refuse to proceed when any file of the layer carries an error diagnostic (accepted by every subcommand) |
| `--share` | `<DIR>` | — | The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE, then the repository's own share/, then the one beside the executable (accepted by every subcommand) |
| `--format` | `text` \| `json` | `text` | Output shape (accepted by every subcommand) — `text`: Lines for a person; `json`: One JSON document, deterministic |

Examples:

- **The live channel's contract, from the types that implement it** — Where the WebSocket is, how a subscription and a reconnect are expressed, every message type, and the JSON Schema of each — derived from the Rust types, so a client validating against this is validating against the implementation. OpenAPI cannot describe a socket, which is why this is a capability and not a paragraph.

  ```console
  $ majordomus executions protocol --format json
  ```

  Verified: exits 0; prints one JSON document carrying /protocol_version, /websocket, /event_types/0, /stream_types/0, /limits/max_events.

