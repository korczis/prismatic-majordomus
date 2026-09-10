+++
title = "Public API quality"
description = "what the Rust crate's exported surface is held to: documentation that says more than the signature, an executable example on everything that carries behaviour, a module boundary something exercises, and every command of the command line accounted for against the capability registry — the codes, the ratchet, and the golden path for adding a command"
weight = 49
[extra]
source = "docs/QUALITY.md"
+++

{% raw %}

What the Rust crate's exported surface is held to, how it is measured, and what to do when
the gate says no.

The rules are [`project.rust-public-api-quality`](../.ai/repo/rules/project/rust-public-api-quality.v1.md)
and [`project.operation-transport-parity`](../.ai/repo/rules/project/operation-transport-parity.v1.md);
the decision behind them is [ADR 0023](../.ai/repo/adrs/0023-the-crates-public-surface-is-measured-by-the-crate-and-absence.md).
This page is the working reference: the rules say why, this says how.

## The one command

```sh
majordomus quality report              # every finding, grouped by code, with the remedy
majordomus quality report --summary    # the counts alone
majordomus quality report --format json
majordomus quality report --code RUST_MODULE_MISSING_EXAMPLE
majordomus quality report --path apps/majordomus-cli/src/web
```

Exit `0` when nothing stands outside the baseline, `10` otherwise — the code every unmet
contract in this executable uses. The JSON and the terminal rendering are two readings of
one execution, so they cannot disagree about the outcome.

The same measurement answers at `GET /api/v1/quality`, as the MCP tool `majordomus_quality`
and the resource `majordomus://quality`, and on the Cockpit's **Quality** page. It is a
capability like any other; nothing on that list is a second implementation.

## What is asked, and of what

<div class="overflow-x-auto" tabindex="0">

| Item | Documented | Says more than the signature | Executable example | Exercised by a test |
|------|-----------|------------------------------|--------------------|---------------------|
| module | yes | yes (a boundary takes explaining) | yes | yes |
| struct, enum, union, trait | yes | yes | yes | — |
| function, method, exported macro | yes | yes | yes | — |
| field, variant, constant, static, type alias | yes | — | — | — |
| accessor that hands back what the receiver holds | yes | — | — | — |

</div>


The last two rows are not exemptions anybody granted. They are derived from the item's kind
and shape, and the report prints them with the reason, so what is not asked for is as
visible as what is. There is no exemption file.

**Exported** means what `rustc` means: declared `pub`, in a chain of `pub` modules from the
crate root, or carried out of a private one by a `pub use`. A `pub` item in a private module
is not measured, which is the first thing to reach for when the gate asks for an example you
do not want to write.

## What counts as an example

A fenced block counts when all three are true:

1. **The toolchain compiles it.** No `ignore`. `no_run` counts — the distinction is whether
   it was compiled, not whether it was run. A ```` ```text ```` block is prose in a box.
2. **It names the item it documents.** An example of `Capability` that never writes
   `Capability` is evidence about something else.
3. **It asserts something that could be false.** `unwrap`, `expect` and `?` are assertions,
   because a failure of any of them fails the doctest. `assert!(true)`, `assert_eq!(1, 1)`
   and `assert_eq!(2 + 2, 4)` are not.

An item with one real example and three diagrams is clean: the best block decides.

## When the gate says no

Every finding carries its code, the rule that requires it, the file and line, the symbol,
why it matters and what to do:

```text
RUST_PUBLIC_MISSING_EXAMPLE  (3 finding(s))
  rule        project.rust-public-api-quality@1
  why         prose about behaviour drifts; an executable example cannot, because the toolchain runs it
  remedy      add a ``` block that calls the item and asserts the result; or reduce the
              item's visibility to pub(crate) if no consumer needs it
  apps/majordomus-cli/src/graph.rs:42: … majordomus_cli::graph::build — …
```

The second half of that remedy is the one people forget. This crate is `publish = false`:
its consumers are its own binary, its tests, its benches and its doc examples. An item none
of them names is not an API, and `pub(crate)` is a complete answer to the finding.

### The codes

<div class="overflow-x-auto" tabindex="0">

| Code | What it means |
|------|---------------|
| `RUST_PUBLIC_MISSING_DOCS` | An exported item carries no documentation. |
| `RUST_PUBLIC_THIN_DOCS` | Its documentation repeats the signature. |
| `RUST_PUBLIC_MISSING_EXAMPLE` | It carries behaviour and no example of it. |
| `RUST_EXAMPLE_NOT_EXECUTABLE` | Every block is `ignore`d, or is prose in a fence. |
| `RUST_EXAMPLE_PLACEHOLDER` | Every block asserts only what is true of any program. |
| `RUST_EXAMPLE_DOES_NOT_NAME_SUBJECT` | No block names the item it documents. |
| `RUST_MODULE_MISSING_DOCS` | No `//!` header, or one that names the boundary without explaining it. |
| `RUST_MODULE_MISSING_EXAMPLE` | The header carries no executable example. |
| `RUST_MODULE_MISSING_BEHAVIOURAL_TEST` | Nothing exercises the module. |
| `OPERATION_CLI_UNCLASSIFIED` | A runnable command is neither a capability nor classified. |
| `OPERATION_CLASSIFICATION_STALE` | A classification names a command or capability that is not there. |
| `OPERATION_CLASSIFICATION_CONFLICT` | A command is both bound to a capability and classified. |
| `OPERATION_MISSING_OPENAPI` | A declared route the document does not describe. |
| `OPERATION_PROJECTION_MISSING` | A declared projection that does not exist. |
| `OPERATION_PROJECTION_ORPHAN` | A projection entry no capability declares. |

</div>


Codes are stable. One is never renamed once it has shipped.

## The ratchet

`.ai/repo/rust-quality-baseline.txt` records the findings that stood when the rule landed.
The gate fails for a finding that is **not** in it, so the debt can shrink and cannot grow;
and it fails for an entry naming a finding that no longer exists, so paid debt has to leave
the file.

Keys are `CODE<TAB>path<TAB>symbol`. The line is deliberately not part of the key: a finding
that moves down its file when something above it is edited is the same finding, and a
baseline keyed by line would turn every unrelated edit into new debt.

```sh
majordomus quality report --write-baseline
```

is the only thing that writes it, and it is a deliberate act. Run it when you have *paid*
debt, not when you have found new debt — the whole point is that new debt fails. The rule is
finished when the file is empty and removed.

## Adding a command: the golden path

A command is the projection of a capability, or it says why it is not. Both are checked.

**A capability** — the normal case, and the one that gets you every projection at once:

1. Declare the typed input and output where the module can see them, deriving
   `serde::Serialize`, `serde::Deserialize` and `schemars::JsonSchema`, and implement
   `BenchmarkCases` for the input.
2. Write the handler: `fn (&Context, Input) -> Result<Output, CapabilityError>`.
3. Add a `capability! { … }` to the module's `module()`, declaring the exposures it wants —
   including `cli: Some(CliExposure { path: vec!["thing".into(), "show".into()] })`.
4. Assert in that same file that the declaration yields the identity and the projections it
   claims (`project.rust-command-tested-in-file`).
5. Declare the clap command in `src/cli.rs` and dispatch it through
   `Context::execute(<id>, input)` — the command owns the rendering and nothing else.
6. `majordomus generate` to refresh the committed projections.

MCP, HTTP, OpenAPI, Swagger, the Cockpit, the benchmark targets and the generated reference
follow from step 3. Nothing else is registered anywhere.

**Not a capability** — add an entry to `cli::LOCAL` in `src/cli/local.rs` with one of:

<div class="overflow-x-auto" tabindex="0">

| Reason | When |
|--------|------|
| `ProcessLifecycle` | What it produces is a running process, not a value. |
| `WritesRepository` | It writes into the repository; every projection of the registry is read-only. |
| `SessionLocal` | It answers about the working copy and shell that invoked it. |
| `Alias(target)` | It runs a default subcommand and is that command's other name. |
| `RendersCapability(id)` | It renders, for a person, the value that capability answers. |

</div>


`RendersCapability` is checked: the capability must exist and must answer over HTTP, so the
operation really is in the API and the command really is its rendering. There is no reason
meaning *not projected yet*, and that is on purpose.

## Where the parts are

```text
src/quality/source.rs   the crate as a syntax tree: what is exported, where, with what docs
src/quality/policy.rs   what an item owes, and what counts as having paid it
src/quality/parity.rs   the registry against MCP, HTTP, OpenAPI and the command line
src/quality/report.rs   one measurement: counts and findings, sorted, deterministic
src/quality/model.rs    the vocabulary: codes, severities, the report's shape
src/cli/local.rs        why a command is not a capability
src/capability/builtin/quality.rs   the capability every projection reads
tests/quality.rs        the ratchet, and one fixture per way of failing
```

The measurement is deterministic and needs no network: file reads are its only I/O, and two
runs over one tree produce equal reports.
{% endraw %}
