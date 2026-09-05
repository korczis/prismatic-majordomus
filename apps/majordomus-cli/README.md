# majordomus-cli

The Rust executable of Majordomus. It builds a binary named `majordomus` that reads the
repository's provider-neutral AI layer under `.ai/`, composes one capability registry from
its own executable capabilities and the layer's objects, and serves that registry through
several interfaces, all read-only: MCP over stdio (`mcp`), HTTP with an OpenAPI document
and a Swagger UI shell on the loopback interface (`serve`), introspection on the command
line (`capabilities`), and generated reference files (`generate`).

It is not a second implementation of the shell tool: `init`, `start`, `check`, `finish`,
`doctor`, `update` and the task lifecycle live in `bin/majordomus` at the repository root,
and this executable advertises only the commands it implements. The architecture is
[`docs/CAPABILITIES.md`](../../docs/CAPABILITIES.md); the MCP surface as a client sees it
is [`docs/MCP.md`](../../docs/MCP.md); the decisions are ADR 1 and ADR 2 under
`.ai/repo/adrs/`.

## What belongs here, and what does not

Belongs: reading the layer as the repository declares it, validating what it reads against
the schemas of the distribution and the repository, composing the registry, projecting it,
and saying precisely what could not be read. Every capability is read-only and every start
leaves the repository byte-identical.

Does not belong: any mutation of the repository, the task lifecycle, projections of the
policy into provider files, hooks, a daemon, authentication, a database, model invocation,
or anything Prismatic. The binary depends on no service and no other repository; it needs
the tool distribution's `share/` directory at run time and `git` for the default discovery.

## Build, run, test

```bash
cargo build --manifest-path apps/majordomus-cli/Cargo.toml
B=apps/majordomus-cli/target/debug/majordomus
$B --help
$B --version
$B mcp --inspect                  # what would be served, every diagnostic; exit 10 when degraded
$B mcp                            # MCP on stdio until the client goes
$B serve                          # HTTP on 127.0.0.1:8741; /openapi.json, /docs, /api/v1/...
$B capabilities list              # every capability with its projections
$B capabilities describe objects.get
$B capabilities schema objects.search --side input
$B capabilities validate          # the registry's invariants and every projection; exit 10 with the list
$B generate                       # docs/generated/*.{json,md} and share/allow/*.txt from the registry and the schemas
$B generate --check               # exit 10 naming every stale file; writes nothing
```

Rust 1.85 or newer, `git` on `PATH` for the default discovery, and a share directory
(below). From `apps/majordomus-cli/`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings   # missing_docs is an error here
cargo test                                                  # unit, integration, doctests
cargo doc --no-deps                                         # RUSTDOCFLAGS="-D warnings" in CI
cargo bench                                                 # criterion, benches/projections.rs
cargo llvm-cov --all-targets --summary-only                 # coverage; CI enforces the threshold in scripts/rust-check
```

`scripts/rust-check` at the repository root runs all of it in the order CI does. The tests
build a disposable repository each and never read this checkout; `test/cases/72_rust_mcp.sh`
and `76_capabilities_projections.sh` at the repository root run the built binary against a
repository the shell tool's own `init` wrote, which is the cross-check that the two
executables read the same layer.

The binary shares its name with the shell tool on purpose. Put one or the other on `PATH`,
or call this one by path; an MCP client configuration names the path anyway:

```json
{ "mcpServers": { "majordomus": { "command": "/path/to/apps/majordomus-cli/target/debug/majordomus", "args": ["mcp"], "env": { "MAJORDOMUS_SHARE": "/path/to/prismatic-majordomus/share" } } } }
```

## The share directory: kinds and schemas at run time

Nothing about kinds or keys is compiled in. On every start the executable locates a share
directory and reads `kinds.yaml` and `schemas/<name>.schema.json` from it, in this order:

| source | when |
|---|---|
| `--share <DIR>` | always wins; a directory without `kinds.yaml` is an error |
| `MAJORDOMUS_SHARE` | same |
| `<repository root>/share` | when it holds `kinds.yaml`: this repository supervising itself |
| `<directory of the executable>/../share` | an installation laid out as `bin/` beside `share/` |

None found: exit 12, every directory tried named. A repository adds kinds under
`.ai/repo/knowledge/kinds.yaml` and schemas under `.ai/repo/knowledge/schemas/`; a name the
distribution already declares is an error naming both files.

## Command reference

Every command starts from the same options:

| option | effect |
|---|---|
| `--repo <PATH>` | start the root search at `PATH` instead of the working directory; an invalid path is an error, never a fallback |
| `--discovery vcs\|filesystem` | `vcs` (default) enumerates tracked files through `git ls-files`, the layer's contract; `filesystem` walks the work tree with the same glob semantics and sees untracked files too |
| `--strict` | exit 10 instead of proceeding when any file of the layer carries an error diagnostic |
| `--share <DIR>` | the share directory, see above |

### `majordomus mcp`

| | |
|---|---|
| stdout | protocol frames only, one JSON-RPC message per line; nothing else, ever |
| stderr | structured diagnostics through `tracing`, filtered by `MAJORDOMUS_LOG` (default `info`) |
| stdin | protocol frames, one per line; EOF ends the session |
| side effects | none |
| exit | `0` at EOF or when the client closes its read end |
| `--inspect [--format text\|json]` | print the repository, resources, tools and every diagnostic, then exit; 10 when there is an error diagnostic |
| `--transport stdio` | the only transport; the option exists so that a second one is an addition |

### `majordomus serve`

| | |
|---|---|
| bind | `--host` (default `127.0.0.1`) and `--port` (default `8741`; `0` picks a free port and the address is logged on stderr) |
| routes | `/` (an index), `/openapi.json`, `/docs`, and one route per capability with an HTTP exposure under `/api/v1/`; `HEAD` answers like `GET` without a body |
| binding | `GET` binds every top-level input property as a query parameter coerced by its schema type; `POST` binds the JSON body (no builtin uses it yet) |
| errors | JSON `{ "error": { "code", "message" } }`: 400 `invalid_input`, 404 `not_found`, 405 `method_not_allowed`, 413 `too_large`, 422 `refused`, 500 `internal` |
| lifecycle | when stdin is a pipe or a socket the server stops at its end of file; otherwise (a terminal, `/dev/null`, a file) it runs until the process is stopped |
| side effects | none; nothing is written, no state is kept between requests |
| exit | `0` when stopped through stdin; `13` when the port cannot be bound |

Swagger UI's own assets are loaded by the browser from the pinned `swagger-ui-dist` on
unpkg; the document it renders is local. That is the one part of the HTTP projection that
needs the network.

### `majordomus capabilities list|describe|schema|validate`

`list [--kind query|resource] [--exposure mcp|http|cli] [--format text|json]` and
`describe <id> [--format …]` dispatch through the registry's own introspection
capabilities, bound by their CLI exposure; `schema <id> [--side input|output]` prints a
canonical schema; `validate` builds the registry and every projection and prints one `OK`
line per check, or exits 10 with every violation named. An unknown id exits 12.

### `majordomus generate [all|openapi|docs|allow] [--check] [--out <DIR>]`

Writes `docs/generated/openapi.json`, `docs/generated/capabilities.md` and, from the
schemas, `share/allow/*.txt`; `--check` compares without writing and exits 10 naming every
stale file; `--out` redirects the repository-relative artifacts under another root.

### Exit codes

The same contract as `docs/CLI.md`:

| code | meaning | examples |
|---|---|---|
| `0` | ok | served until EOF; `--inspect` or `validate` with nothing wrong |
| `2` | usage | unknown command or option (clap) |
| `10` | contract unmet | manifest, `sources.yaml`, kinds or a schema invalid; the registry does not build; `--strict` with errors; `--inspect` with errors; stale artifacts under `--check` |
| `12` | missing artifact | no `.ai/manifest.yaml` in any ancestor; the pre-`.ai` layout; no share directory; an unknown capability id |
| `13` | internal | I/O failure, `git` unusable under `--discovery vcs`, a port that cannot be bound, transport failure |

### Side-effect table

| command | filesystem mutation | git mutation | network | stdout |
|---|---|---|---|---|
| `--help`, `--version`, `capabilities …`, `mcp --inspect`, `generate --check` | no | no | no | text or JSON |
| `mcp` | no | no | no | MCP protocol |
| `serve` | no | no | listens on loopback; the browser fetches Swagger UI assets | HTTP on the socket, nothing on stdout |
| `generate` (without `--check`) | writes `docs/generated/` and `share/allow/` | no | no | the paths written |

`tests/mcp_stdio.rs::serving_mutates_nothing` and
`tests/http_serve.rs::serving_from_a_nested_directory_finds_the_same_root_and_writes_nothing`
compare `git status` and every tracked blob before and after a session.

## Architecture

```text
main.rs               parse, init stderr logging, run, map the error to an exit code
cli.rs                clap declarations; RepoArgs shared by every command
app.rs                the one composition point: repository -> share -> kinds and schemas -> index -> registry
commands/             mcp, serve, capabilities, generate: each a function from its arguments to an exit code
repository.rs         root discovery (nearest .ai/manifest.yaml), the typed manifest
share.rs              locating the distribution's share directory; reading its schemas
discovery/            sources.yaml, the DiscoverySource trait, its two implementations, :(glob) matching
metadata/             kinds.yaml, the schema set (jsonschema), the YAML subset, front matter
index.rs              read every discovered file into an Object or a Diagnostic; dedupe; sort
model.rs              Object, Provenance, Diagnostic, Severity: the domain of the layer, no I/O
capability/           the canonical model: model.rs (descriptor), schema.rs (canonical schemas and their
                      MCP and OpenAPI projections), handler.rs (typed handlers, Context, the capability! macro),
                      registry.rs (the registry and its invariants), builtin.rs (the executables), declarative.rs
mcp/                  surface.rs (the registry as resources and tools), protocol.rs (JSON-RPC and MCP), stdio.rs
http/                 router.rs (routes and binding from the registry), openapi.rs, swagger.rs, server.rs (tiny_http)
generate.rs           the one generator pipeline and the allow-list derivation
git/                  read-only git: toplevel, head, branch, dirty state, ls-files
logging.rs, error.rs  tracing to stderr; the typed errors and their exit codes
```

Dependencies flow inward: `commands` knows clap and everything below; `mcp` and `http`
know the registry and nothing about clap or files; `capability` knows the index's model;
`index`, `discovery`, `metadata`, `repository`, `share` and `git` know the model and the
errors; `model` knows nothing. No MCP library and no HTTP framework: the read-only subset
of MCP is eight methods in one file, and six loopback routes need a synchronous accept
loop, not an async runtime. The one trait, `DiscoverySource`, exists because two
enumerations ship. Dependencies: `clap`, `serde`, `serde_json`, `schemars`, `jsonschema`,
`tiny_http`, `thiserror`, `tracing`, `tracing-subscriber`; dev: `tempfile`, `criterion`.

## Repository discovery

Walk from the start directory upward. The first ancestor carrying `.ai/manifest.yaml` is
the root. A manifest that does not parse, carries a key nothing reads, or declares a schema
other than `ai-repository/v1` stops the search with that error: a nearer broken layer is
never skipped for a farther working one. `.git` is not a marker, so an ordinary git
repository is not a Majordomus repository. `.majordomus/` is not a marker either: with
`bin/majordomus` inside it is an installation of the tool and ignored; without one it is
the pre-`.ai` layout and refused by name, pointing at `majordomus migrate`.

## Data-driven model

Nothing in the Rust code names a repository file except the two bootstrap conventions the
layer itself documents, `.ai/manifest.yaml` and `sources.yaml` under the manifest's
`knowledge` section, and the file names of the distribution (`kinds.yaml`,
`schemas/*.schema.json`). Everything else is read from data at run time:

| decides | read from | owner |
|---|---|---|
| where the layer is | `.ai/manifest.yaml` exists | repository |
| which sections exist | `sections:` in the manifest | repository |
| which file names must carry the context contract | `context.documents:` in the manifest | repository |
| which files carry which kind | `.ai/repo/knowledge/sources.yaml` | repository |
| how a kind is read | `share/kinds.yaml`, plus `.ai/repo/knowledge/kinds.yaml` | distribution, repository |
| which keys and values a kind may carry | `share/schemas/<kind>.schema.json`, plus `.ai/repo/knowledge/schemas/` | distribution, repository |

The YAML reader implements the layer's documented subset (`docs/SCHEMAS.md`), not general
YAML, so this executable and the shell tool accept the same files and refuse the same ones
(one deliberate divergence: this reader trims trailing whitespace before testing quotes;
the shell keeps the quotes and refuses such a prompt downstream). Failure policy, decided
once in `index.rs`: the manifest, `sources.yaml`, the kinds files and the schemas are
errors, because without them nothing can be discovered; every other file that cannot
become an object is excluded with an error diagnostic naming its path and code, and the
index is `degraded`. A degraded index still serves; `--strict` refuses it.

Diagnostic codes: `malformed_front_matter`, `missing_front_matter`, `malformed_yaml`,
`unknown_key`, `schema_violation`, `kind_mismatch`, `unsupported_version`,
`missing_field`, `missing_context_contract`, `duplicate_identity`, `unknown_kind`,
`required_source_empty`, `claimed_twice` (warning), `invalid_utf8`, `oversized`,
`symlink`, `not_a_file`, `unreadable`.

## Security boundaries

Repository content is untrusted input: symlinks are never followed or read; a file over
4 MiB or a front matter over 64 KiB is refused; invalid UTF-8 is refused; only paths the
declared pathspecs match are read, and only from the repository root; `resources/read` and
`objects.get` answer from the in-memory index, never from a path a client names; the HTTP
server binds loopback by default, reads at most 1 MiB of body, and trusts no header. Not
defended against: a hostile `sources.yaml` naming a huge tree (the walk is bounded by the
repository, not by a budget), and memory, since every object's content is held for the
session.

## Stability

| | status |
|---|---|
| `--help`, `--version`, `mcp --help`, exit codes, the new commands' help and codes | behaviourally verified (`tests/cli.rs`) |
| root discovery, nearest-wins, legacy refusal, `--repo` | behaviourally verified (`tests/repository_discovery.rs`) |
| the kind schema, run-time schemas, every diagnostic code, repository-defined kinds | behaviourally verified (`tests/metadata_contract.rs`, `tests/external_extension.rs`) |
| determinism across enumerations, provenance, classification | behaviourally verified (`tests/index_behavior.rs`) |
| MCP: handshake, listing, reads, tools, errors, clean EOF, protocol-only stdout, no mutation | behaviourally verified (`tests/mcp_stdio.rs`) |
| the registry's invariants; every projection present and none orphan; a change reaches every projection | behaviourally verified (`tests/registry.rs`, `tests/projections.rs`) |
| HTTP over a socket, OpenAPI, Swagger shell, typed errors, HEAD, `/dev/null` stdin, MCP/HTTP parity | behaviourally verified (`tests/http_serve.rs`) |
| `generate`, byte-identical regeneration, `--check` | behaviourally verified (`tests/generate_check.rs`) |
| the layer `init` writes, served with state `ok`; one capability through every interface | behaviourally verified (`test/cases/72_rust_mcp.sh`, `76_capabilities_projections.sh`) |
| CLI names, URIs, tool names, routes, diagnostic codes, `kinds.yaml`, the schema files | implemented; pre-1.0, changes are documented, never silent |
| `--discovery filesystem` | implemented; a convenience outside the layer's contract |

Deferred, deliberately: MCP prompts (prompt assets render `{{CONTEXT}}` from local state
the shell tool owns), a second transport, mutation over any interface, subscriptions and
list-change notifications, path parameters and POST capabilities, `/openapi.yaml`, Swagger
UI assets offline, hot reload, the `.ai/local/` half, a content hash per object.
