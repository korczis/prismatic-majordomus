<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the clap declaration of the command line and the examples declared with it (apps/majordomus-cli/src/cli.rs);
     regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Command line of the Rust executable

Majordomus control plane: a data-driven MCP server over the repository's .ai/ layer

The Rust executable of Majordomus. It reads the repository's provider-neutral AI layer under .ai/ and serves it, read-only, to MCP clients over stdio.

The task lifecycle (init, start, check, finish, doctor, ...) is the shell tool bin/majordomus in the same repository; this executable does not implement those commands.

Every command below is declared once, in [`apps/majordomus-cli/src/cli.rs`](../../apps/majordomus-cli/src/cli.rs), together with its examples; this file is a projection of that declaration, as `--help` is, as `docs/generated/cli.json` is, and as the website's reference under `/docs/cli/` is. Every example printed here is executed against the built executable by `apps/majordomus-cli/tests/cli_examples.rs`. The task lifecycle (`init`, `start`, `check`, `finish`, `doctor`, ...) is the *shell* tool `bin/majordomus`, a different program, documented in `docs/CLI.md`.

## Commands

| command | route | does |
|---|---|---|
| [`majordomus mcp`](#majordomus-mcp) | `/docs/cli/mcp/` | Serve the repository's AI layer to an MCP client over stdio (read-only) |
| [`majordomus serve`](#majordomus-serve) | `/docs/cli/serve/` | Serve the same capabilities over HTTP on the loopback interface, with /openapi.json and /docs (read-only) |
| [`majordomus capabilities`](#majordomus-capabilities) | `/docs/cli/capabilities/` | Introspect the capability registry: what exists, where it came from, how it is exposed |
| [`majordomus capabilities list`](#majordomus-capabilities-list) | `/docs/cli/capabilities/list/` | Every capability, one line each, with its projections |
| [`majordomus capabilities describe`](#majordomus-capabilities-describe) | `/docs/cli/capabilities/describe/` | One capability by canonical id: schemas, provenance, every projection |
| [`majordomus capabilities schema`](#majordomus-capabilities-schema) | `/docs/cli/capabilities/schema/` | The canonical input or output JSON Schema of one capability |
| [`majordomus capabilities validate`](#majordomus-capabilities-validate) | `/docs/cli/capabilities/validate/` | Build the registry and every projection; exit 10 with every violation named |
| [`majordomus generate`](#majordomus-generate) | `/docs/cli/generate/` | Write the committed projections of the registry (docs/generated), or check that they are current |
| [`majordomus bench`](#majordomus-bench) | `/docs/cli/bench/` | Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations), report coverage, compare with the accepted baseline |
| [`majordomus bench coverage`](#majordomus-bench-coverage) | `/docs/cli/bench/coverage/` | Every required target and whether it is covered; the denominator is generated from the registry |
| [`majordomus bench baseline`](#majordomus-bench-baseline) | `/docs/cli/bench/baseline/` | The accepted baseline of this platform under .ai/repo/benchmarks/rust/ |
| [`majordomus bench baseline update`](#majordomus-bench-baseline-update) | `/docs/cli/bench/baseline/update/` | Run the benchmarks and record them as this platform's baseline (a reviewable, tracked file) |
| [`majordomus scope`](#majordomus-scope) | `/docs/cli/scope/` | The repository scope: what a worker reads and what it never reads; with paths, whether each is in or out and why |

<a id="majordomus"></a>
## `majordomus`

Majordomus control plane: a data-driven MCP server over the repository's .ai/ layer

Subcommands: [`majordomus mcp`](#majordomus-mcp), [`majordomus serve`](#majordomus-serve), [`majordomus capabilities`](#majordomus-capabilities), [`majordomus generate`](#majordomus-generate), [`majordomus bench`](#majordomus-bench), [`majordomus scope`](#majordomus-scope).

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

Serve the same capabilities over HTTP on the loopback interface, with /openapi.json and /docs (read-only)

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

Examples:

- **Serve the same capabilities over HTTP on a free port** — Port 0 asks the operating system for a free port; the address is logged on stderr. The document at /openapi.json is the same one `majordomus generate` commits, and /docs is the Swagger UI over it.

  ```console
  $ majordomus serve --port 0
  ```

  Verified: binds a port, answers GET /openapi.json, exits 0 when stopped.

<a id="majordomus-capabilities"></a>
## `majordomus capabilities`

Introspect the capability registry: what exists, where it came from, how it is exposed

Subcommands: [`majordomus capabilities list`](#majordomus-capabilities-list), [`majordomus capabilities describe`](#majordomus-capabilities-describe), [`majordomus capabilities schema`](#majordomus-capabilities-schema), [`majordomus capabilities validate`](#majordomus-capabilities-validate).

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
| `<TARGET>` | `all` \| `openapi` \| `docs` \| `benchmarks` \| `registry` \| `allow` \| `providers` \| `site` | `all` | What to generate — `all`: Every target; `openapi`: `docs/generated/openapi.json`; `docs`: `docs/generated/capabilities.md` and `docs/generated/modules/<id>.md`; `benchmarks`: `docs/generated/benchmarks.md`: every benchmark target and the coverage; `registry`: `docs/generated/registry.json`: the builtin registry as data; `allow`: The shell tool's allow-lists under share/allow, derived from the schemas; `providers`: The provider bootstraps the policy declares (AGENTS.md, CLAUDE.md, ...); `site`: site/data/registry/registry.json, the registry dataset the site renders |
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

