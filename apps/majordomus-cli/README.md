# majordomus-cli

The Rust executable of Majordomus. It builds a binary named `majordomus` whose one
command, `mcp`, serves the repository's provider-neutral AI layer under `.ai/` to an MCP
client over stdio, read-only.

It is the control plane's executable surface for AI clients and scripts. It is not a
second implementation of the shell tool: `init`, `start`, `check`, `finish`, `doctor`,
`update` and the rest live in `bin/majordomus` at the repository root, and this
executable does not advertise commands it does not implement.

## What belongs here, and what does not

Belongs: reading the layer as the repository declares it, validating what it reads,
serving it to a client, and saying precisely what could not be read. Every capability is
read-only and every startup leaves the repository byte-identical.

Does not belong: any mutation of the repository, the task lifecycle, projections, hooks,
a daemon, a network transport, a database, model invocation, or anything Prismatic. The
binary depends on no service and no other repository.

## Build, run, test

```bash
cargo build --manifest-path apps/majordomus-cli/Cargo.toml
apps/majordomus-cli/target/debug/majordomus --help
apps/majordomus-cli/target/debug/majordomus --version
apps/majordomus-cli/target/debug/majordomus mcp --help
apps/majordomus-cli/target/debug/majordomus mcp --inspect        # what would be served, and every diagnostic
apps/majordomus-cli/target/debug/majordomus mcp                  # serve on stdio until the client goes
```

The crate is built from inside this repository: it embeds `share/allow/*.txt` at build
time (see "Data-driven model"), so a copy of `apps/majordomus-cli/` alone does not build.
Rust 1.85 or newer and `git` on `PATH`.

```bash
cd apps/majordomus-cli
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The tests need `git`; they build a disposable repository each and never read this
checkout. `test/cases/72_rust_mcp.sh` at the repository root runs the built binary against
a repository the shell tool's own `init` wrote, which is the cross-check that the two
executables read the same layer.

The binary shares its name with the shell tool on purpose. Put one or the other on
`PATH`, or call this one by path; an MCP client configuration names the path anyway:

```json
{ "mcpServers": { "majordomus": { "command": "/path/to/apps/majordomus-cli/target/debug/majordomus", "args": ["mcp"] } } }
```

## Command reference

### `majordomus mcp`

| | |
|---|---|
| purpose | discover the repository, build the index, serve it over stdio |
| stdout | protocol frames only, one JSON-RPC message per line; nothing else, ever |
| stderr | structured diagnostics through `tracing`, filtered by `MAJORDOMUS_LOG` |
| stdin | protocol frames, one per line; EOF ends the session |
| side effects | none: no file written, no git state touched, nothing generated |
| exit | `0` at EOF or when the client closes its read end; see the exit-code table for the rest |

Options:

| option | effect |
|---|---|
| `--repo <PATH>` | start the root search at `PATH` instead of the working directory; an invalid path is an error, never a fallback |
| `--discovery vcs\|filesystem` | `vcs` (default) enumerates tracked files through `git ls-files`, which is the layer's contract; `filesystem` walks the work tree with the same glob semantics and sees untracked files too |
| `--strict` | exit `10` instead of serving when any file of the layer carries an error diagnostic |
| `--inspect` | print the repository, the resources, the tools and every diagnostic, then exit without serving; exit `10` when there is an error diagnostic |
| `--format text\|json` | the shape of `--inspect` output; ignored otherwise, because the protocol owns stdout |
| `--transport stdio` | the only transport; the option exists so that a second one is an addition |

Environment:

| variable | purpose | default |
|---|---|---|
| `MAJORDOMUS_LOG` | the `tracing` filter for stderr, e.g. `debug`, `trace`, `majordomus_cli=debug` | `info` |

### Exit codes

The same contract as `docs/CLI.md`, used for the same meanings:

| code | meaning | examples |
|---|---|---|
| `0` | ok | served until EOF; `--inspect` with no error diagnostic |
| `2` | usage | unknown command or option (clap) |
| `10` | contract unmet | manifest or `sources.yaml` invalid, unknown key, unsupported schema, `--strict` with errors, `--inspect` with errors |
| `12` | missing artifact | no `.ai/manifest.yaml` in any ancestor; the pre-`.ai` layout under `.majordomus/` |
| `13` | internal | I/O failure, `git` unusable under `--discovery vcs`, transport failure |

### Side-effect table

| command | filesystem mutation | git mutation | network | stdout |
|---|---|---|---|---|
| `majordomus --help` / `--version` | no | no | no | help / version |
| `majordomus mcp --inspect` | no | no | no | text or JSON report |
| `majordomus mcp` | no | no | no | MCP protocol |

`tests/mcp_stdio.rs::serving_mutates_nothing` holds the first three rows of the last
column's neighbours to their word: `git status` and every tracked blob are compared before
and after a session.

## Architecture

```text
main.rs            parse, init stderr logging, run, map the error to an exit code
cli.rs             clap declarations: Cli, Command, McpArgs, OutputFormat, DiscoveryMode, Transport
commands/mcp.rs    the one command: discover -> load sources -> build index -> serve or inspect
repository.rs      root discovery (nearest .ai/manifest.yaml), the typed manifest
discovery/         sources.yaml, the DiscoverySource trait, its two implementations, :(glob) matching
metadata/          the embedded kind schema, the allow-lists, the YAML subset, front matter
index.rs           read every discovered file into an Object or a Diagnostic; dedupe; sort
model.rs           Object, Provenance, Diagnostic, Severity: the domain, no I/O
git/               read-only git: toplevel, head, branch, dirty state, ls-files
mcp/surface.rs     the index as resources and tools; every tool answers from the index alone
mcp/protocol.rs    JSON-RPC and the MCP methods over a Surface; the only place wire shapes live
mcp/stdio.rs       one frame per line in, one per line out
logging.rs         tracing to stderr
error.rs           the typed errors and their exit codes
```

Dependencies flow inward: `commands` knows clap and everything below; `mcp` knows the
index and nothing about clap or files; `index`, `discovery`, `metadata`, `repository` and
`git` know the model and the errors; `model` knows nothing. There is no MCP crate: the
protocol subset a read-only server needs is small, and keeping it in one file means no
third-party type reaches the domain. Replacing `protocol.rs` and `stdio.rs` with a
library would touch nothing else.

The one trait, `DiscoverySource`, exists because two enumerations ship and the caller
chooses one explicitly. No other extension trait exists, because nothing has a second
implementation yet.

## Repository discovery

Walk from the start directory upward. The first ancestor carrying `.ai/manifest.yaml` is
the root. A manifest that does not parse, carries a key nothing reads, or declares a
schema other than `ai-repository/v1` stops the search with that error: a nearer broken
layer is never skipped for a farther working one. `.git` is not a marker, so an ordinary
git repository is not a Majordomus repository. `.majordomus/` is not a marker either: with
`bin/majordomus` inside it is an installation of the tool and ignored; without one it is
the pre-`.ai` layout and refused by name, pointing at `majordomus migrate`.

`tests/repository_discovery.rs` holds every case: root, nested directory, ordinary git
repository, no markers, two nested candidates, a nearer broken manifest, the legacy
layout, an installation, and `--repo`.

## Data-driven model

Nothing in the Rust code names a repository file except the two bootstrap conventions the
layer itself documents: `.ai/manifest.yaml` and `sources.yaml` under the manifest's
`knowledge` section. From there everything is read from data:

| decides | read from | owner |
|---|---|---|
| where the layer is | `.ai/manifest.yaml` exists | repository |
| which sections exist | `sections:` in the manifest | repository |
| which files carry which kind | `.ai/repo/knowledge/sources.yaml`: one pathspec and kind per class | repository |
| which keys a kind may carry | `share/allow/<kind>.txt`, embedded at build | this repository's tool distribution |
| how a kind is read: format, front matter, identity, title | `schema/kinds.yaml`, embedded at build | this crate |

`schema/README.md` is the contract for the last two rows. The YAML reader implements the
layer's documented subset (`docs/SCHEMAS.md`), not general YAML, so this executable and
the shell tool accept the same files and refuse the same ones.

Pipeline, once per invocation:

```text
Repository::discover -> Sources::load -> discover (sorted paths per class, local half excluded)
  -> read_object per file (format, front matter, allow-list, kind, version, identity, title)
  -> dedupe identities -> sort by URI -> Index { objects, diagnostics, state }
  -> Surface (resources, tools) -> Server (protocol) -> stdio
```

Failure policy, decided once in `index.rs`: the manifest and `sources.yaml` are errors,
because without them nothing can be discovered; every other file that cannot become an
object is excluded with an error diagnostic naming its path and code, and the index is
`degraded`. A degraded index still serves; `--strict` refuses it; `--inspect` exits `10`
on it. Nothing is repaired, defaulted or normalised.

Diagnostic codes: `malformed_front_matter`, `missing_front_matter`, `malformed_yaml`,
`unknown_key`, `kind_mismatch`, `unsupported_version`, `missing_field`,
`duplicate_identity`, `unknown_kind`, `required_source_empty`, `claimed_twice` (warning),
`invalid_utf8`, `oversized`, `symlink`, `not_a_file`, `unreadable`.

## MCP surface

Protocol versions accepted: `2025-06-18`, `2025-03-26`, `2024-11-05`; a client asking for
another gets `2025-06-18`. Capabilities advertised: `resources` and `tools`, neither with
`listChanged` or `subscribe`. Prompts are not advertised (see "Deferred").

Resources, one per object plus one for the repository:

| URI | content |
|---|---|
| `majordomus://repository` | JSON: root, layer schema, sections, git state, discovery mode, kinds, every diagnostic |
| `majordomus://<kind>/<identity>` | the file as read; `text/markdown` or `application/yaml` |

Identity is the kind's identity fields joined with `@` (`majordomus.scope-integrity@1`,
`continue`, `implementation`, `M001`) or, for kinds without identity fields, the
repository-relative path (`docs/CLI.md`). Each listed resource carries
`_meta.majordomus` with kind, identity and provenance (path, directory, source class,
manifest section, bytes).

Tools, all read-only and annotated so:

| tool | arguments | answers |
|---|---|---|
| `majordomus_list` | `kind?`, `tag?` | the objects, summarised |
| `majordomus_get` | `uri` | metadata, provenance, media type, content |
| `majordomus_search` | `query`, `kind?`, `limit?` | case-insensitive substring hits with a snippet line |
| `majordomus_repository` | none | the same document as `majordomus://repository` |

A refused call (missing argument, unknown URI) is a result with `isError: true`, not a
protocol error; an unknown tool or method is a protocol error (`-32602`, `-32601`); an
unknown resource is `-32002`; a line that is not JSON is `-32700`. Batches are answered
as batches. Notifications get no answer.

## Extension

For a kind this executable already reads, adding an object is a data change only:

1. write the file where the repository's `sources.yaml` class for that kind looks
   (`.ai/repo/rules/project/<name>.v1.md` for a rule), with the front matter
   `share/allow/rule.txt` allows and the identity fields the kind needs;
2. track it (`git add`), because the layer's contract discovers through the index; or
   run with `--discovery filesystem` to see it untracked;
3. restart the server; the object is listed as `majordomus://rule/<id>@<version>` with
   its own provenance. Remove the file and it is gone. Break it and
   `majordomus_repository` names it.

`tests/external_extension.rs` runs exactly that sequence through the built binary. The
same test shows a second lever: a new class in `sources.yaml` naming a known kind
(`kind: document` for `.ai/repo/adrs/*.md`) is served without any change here.

A genuinely new kind is a Rust-side change by design: an entry in `schema/kinds.yaml`
and, when it has a key contract, an allow-list; a kind needing a new format or identity
rule is code. Data describes objects; it does not define semantics.

## Hierarchy

Every object records the directory it sits in. Root-level `README.md`, `AGENTS.md` and
the other bootstrap files are discovered through the repository's own `readme` class and
served as documents with `directory: "."`; nothing here merges, ranks or composes them,
because the repository defines no merge semantics on `master` today. A client that wants
the hierarchy orders by `provenance.directory`. The unresolved contract is recorded in the
ADR `.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md`.

## Security boundaries

Repository content is untrusted input: symlinks are never followed or read; a file over
4 MiB or a front matter over 64 KiB is refused; invalid UTF-8 is refused; only paths the
declared pathspecs match are read, and only from the repository root; `resources/read`
answers from the in-memory index, never from a path a client names. Not defended against:
a hostile `sources.yaml` naming a huge tree (the walk is bounded by the repository, not by
a budget), and memory, since every object's content is held for the session.

## Stability

| | status |
|---|---|
| `majordomus --help`, `--version`, `mcp --help`, exit codes | behaviourally verified (`tests/cli.rs`) |
| root discovery, nearest-wins, legacy refusal | behaviourally verified (`tests/repository_discovery.rs`) |
| the kind schema and the diagnostics above | behaviourally verified (`tests/metadata_contract.rs`) |
| determinism across enumerations, provenance, classification | behaviourally verified (`tests/index_behavior.rs`) |
| handshake, listing, reads, tools, errors, clean EOF, protocol-only stdout, no mutation | behaviourally verified (`tests/mcp_stdio.rs`) |
| add, remove, break an object with no Rust change | behaviourally verified (`tests/external_extension.rs`) |
| the layer the shell tool's `init` writes is served with state `ok` | behaviourally verified (`test/cases/72_rust_mcp.sh`) |
| CLI names, URIs, tool names, diagnostic codes | implemented; pre-1.0, changes are documented, never silent |
| `--discovery filesystem` | implemented; a convenience outside the layer's contract |

Deferred, deliberately: MCP prompts (prompt assets render `{{CONTEXT}}` from local state
the shell tool owns; serving them unrendered would mislead), any second transport, any
mutation, subscriptions and list-change notifications, the `.ai/local/` half, projection
stamps as provenance, a content hash per object.
