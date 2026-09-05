---
schema: adr/v1
id: adr-0001
kind: adr
title: A Rust executable with a data-driven, read-only MCP surface over stdio
status: accepted
date: 2026-09-05
tags:
  - rust
  - mcp
  - architecture
provenance:
  origin: authored
---

# 1. A Rust executable with a data-driven, read-only MCP surface over stdio

## Context

Majordomus is a shell tool that validates, projects and enforces the repository's `.ai/`
layer. AI clients that speak the Model Context Protocol cannot read a shell tool; they
spawn a process and speak JSON-RPC to it. The repository's own documents refused an "MCP
surface" (README "What this is not", `CONTRIBUTING.md`, `docs/DESIGN.md` deferred list,
`docs/EXTRACTION_REPORT.md`) on the evidence that no daemon, server, database or queue
had solved a problem the file-based mechanisms had not. That evidence stands; what it
argued against was infrastructure, not a reader.

The operator asked for a first-class Rust executable, `majordomus`, whose first command
serves the layer to MCP clients, with the structure of what is served derived from the
repository's data rather than written into the code.

## Decision

- One Rust package, `apps/majordomus-cli/`, building a binary named `majordomus`, with
  `clap` for the command line and one command, `mcp`. `apps/` is new: the executable is a
  product surface, not a helper, so it does not belong under `scripts/` or `bin/`.
- Transport: stdio only, one process per client, alive as long as the client's pipe.
  No port, no daemon, no state, no database, no network.
- Read-only, and proven so: a session leaves `git status` and every tracked blob
  byte-identical.
- Data-driven: the root is the nearest `.ai/manifest.yaml`; the manifest names the
  sections; `.ai/repo/knowledge/sources.yaml` maps pathspecs to kinds and discovery goes
  through the git index as the knowledge contract prescribes; key contracts are the same
  `share/allow/*.txt` the shell tool uses, embedded at build time; the executable ships
  one declarative file of its own, `schema/kinds.yaml`, saying how each kind is read.
- The YAML reader implements the layer's documented subset, not general YAML, so both
  executables accept and refuse the same files.
- No MCP library: the read-only subset a server needs is eight methods, kept in one
  file behind the index, so that no third-party type reaches the domain and a library
  can replace that file later without touching discovery or metadata.
- Failure policy, decided once: the manifest and `sources.yaml` are errors; any other
  file that cannot become an object is excluded with a named diagnostic and the index is
  `degraded` and still serves; `--strict` refuses a degraded index.

## Invariant

Rust carries mechanism: read a format, validate keys against a list, take an identity
from named fields, speak the protocol. The repository carries the knowledge: which files,
which kinds, which keys. Adding an object of a known kind, or a new class of a known kind,
is a data change; adding a kind is a schema entry plus, when the kind needs a new format
or identity rule, code.

## Against the "Intentionally Absent" list, point by point

| the list refuses | this decision |
|---|---|
| a daemon, server, background monitor | none: the process is the client's child and dies with it |
| a database, queue, vector store | none: one in-memory index per invocation |
| a registry or catalogue of named workers | none: the index holds the repository's own declared objects, nothing of its own |
| a finding without a reproduce command | every diagnostic carries a stable code and the path; `mcp --inspect` reproduces it |
| a self-report trusted without an independent check | `test/cases/72_rust_mcp.sh` speaks to the built binary over pipes in a repository `init` wrote |
| a number written where a command could compute it | the executable computes every count it reports |

## Alternatives rejected

- **Extend the shell tool with an MCP command.** JSON-RPC framing, a JSON serialiser and
  a long-lived stdin loop in bash 3.2 would repeat the hand-rolled serialiser the
  extraction report rejected; the shell tool's portability floor is a strength for hooks
  and a liability for a protocol server.
- **Python.** No Python is in the repository's toolchain or CI; adding an interpreter
  dependency for one command contradicts "nothing else" in the requirements.
- **A separate MCP product or daemon.** A second process with its own lifecycle is the
  infrastructure the design refuses, and would need its own reader of the layer.
- **A hardcoded tool and resource table in Rust.** Would drift from the layer on the
  first new rule and is exactly the second source of truth `28_no_hardcoded_values.sh`
  exists to prevent.
- **An MCP crate.** The official Rust SDK is maintained and would work; it brings an
  async runtime, a schema library and macros for a server that needs eight read-only
  methods, and its types would sit where the domain's should. Reversible: the protocol
  file is the only place wire shapes live.
- **A general YAML crate.** Accepts what the layer refuses (anchors, block scalars) and
  refuses what the layer accepts (an unquoted scalar carrying `: `, a backtick-led
  value); the two executables would disagree about the same file.
- **Any Prismatic dependency.** Refused by the clean-room rule; none exists at compile,
  run, deployment or operational level.

## Consequences

Positive: one executable an MCP client can spawn; the surface grows with the layer
without a code change; every claim has a black-box test; provider-neutral by
construction, since provider names appear nowhere in the code.

Negative and accepted: `schema/kinds.yaml`, the resource URI scheme, the tool names and
the diagnostic codes become a compatibility surface; the executable embeds
`share/allow/` and so builds only from inside this repository; two executables now read
the layer, and `72_rust_mcp.sh` is the check that they agree; the documents that refused
an MCP surface now distinguish infrastructure (still refused) from a stdio reader.

## Unresolved, recorded rather than invented

Hierarchical context (root `README.md`, `AGENTS.md`, provider files, and whatever a
subdirectory may carry) has no merge semantics on `master`. The executable records each
document's directory and composes nothing. When the repository defines the contract, the
executable reads it from data like everything else.

## Testing strategy

Black-box first: the compiled binary is spawned for the CLI, discovery, the protocol, and
the add–remove–break extension sequence; the index is exercised through the library only
for determinism across the two enumerations. `test/cases/72_rust_mcp.sh` is the
cross-check against the shell tool's own skeleton, and CI runs `cargo fmt --check`,
`clippy -D warnings` and `cargo test` beside the behavioural suite.
