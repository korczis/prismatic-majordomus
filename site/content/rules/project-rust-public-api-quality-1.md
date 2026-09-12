+++
title = "Every exported Rust item explains itself, shows itself running, and is exercised by something"
description = "Every exported Rust item explains itself, shows itself running, and is exercised by something"
weight = 119
[extra]
kind = "rule"
slug = "project-rust-public-api-quality-1"
identity = "project.rust-public-api-quality@1"
status = "active"
source = ".ai/repo/rules/project/rust-public-api-quality.v1.md"
+++
{% raw %}

## Rationale

The compiler already refuses an exported item with no documentation: `#![warn(missing_docs)]`
in `lib.rs` and `-D warnings` in the gate make the absence a build failure. That catches
nothing about whether the documentation is worth reading, and it catches nothing at all
about examples, which is where the interesting failure lives.

Prose about behaviour drifts. It drifts silently, it drifts in the direction of what the
code used to do, and no gate in this repository could see it — because prose is not run.
An example is run: `cargo test --doc` compiles it against the real signatures and executes
it against the real behaviour, so an example that has gone stale is a red build rather than
a paragraph that misleads the next reader. That is the whole argument for this rule, and it
is the same argument `project.rust-command-tested-in-file` already makes for commands. This
rule generalises it from the command modules to the surface.

The second half is the boundary. A module is a conceptual unit — it owns something, it has
invariants, it has neighbours — and a module that does not say what it owns is a directory
with a `.rs` on the end. When this rule first measured the crate, every exported module had
a header and fewer than one in ten had an example in it, and a quarter of them had no test
beside them and no test naming them. Those are the modules where a change breaks nothing
visible, which is exactly where changes go wrong. The current numbers are not written here:
`majordomus quality report --summary` computes them.

The third part is what the rule does *not* demand, and this is deliberate. A field, a
variant, a constant, a static and a type alias are named and typed; an example of one is an
example of whatever reads it, and a rule that demanded one would be satisfied with
`assert!(true)` inside a fence — the ceremony this rule exists to refuse. The same is true
of an accessor whose body hands back what the receiver already holds. So the policy
classifies every item from its kind and its shape, derives what it may ask of it, and
**prints what it did not ask for, with the reason**. There is no exemption file, because an
exemption file is a place to add a line.

This rule does not replace `project.rust-command-tested-in-file`, and the two have different
subjects. That rule is about the modules the application *composes* — every one of them,
whatever its visibility — and asks for one assertion beside the declaration, in either
accepted form. This one is about what the crate *exports* to anybody, and asks for both an
example and a test. A capability module that is `pub(crate)` is held by the first and not by
the second, which is right: it is still a command, and it is not API.

A strict rule about the exported surface is also a rule about how large that surface is.
This crate is `publish = false`: its consumers are its own binary, its tests, its benches
and its doc examples, and nothing else. An item that no consumer needs costs a doc comment
and an example under this rule, and the right answer to that cost is usually `pub(crate)`.
That is the intended pressure and not a side effect of it.

## Required behaviour

Every item the crate exports — reachable from the crate root through `pub` modules, or
carried out of a private one by a `pub use` — carries documentation. Where the item carries
behaviour (a module, a type, a trait, a function, a method, an exported macro) that
documentation says something the signature does not, and includes at least one fenced block
that the toolchain compiles: no `ignore`, no fenced box of prose standing in for one.

An example counts when it is compiled, when it names the item it is documenting, and when
it asserts something that could be false. `assert!(true)`, `assert_eq!(1, 1)` and an
arithmetic identity are not assertions about the item; `unwrap`, `expect` and `?` are,
because a failure of either fails the doctest. `no_run` counts and `ignore` does not: the
difference is whether the toolchain compiled it.

Every exported module carries a `//!` header that explains the boundary rather than naming
it, at least one executable example of its lifecycle, and behavioural coverage: a
`#[cfg(test)]` test beside the declaration, or a test under `tests/` that names the module.
Either form counts, for the reason `project.rust-command-tested-in-file` gives — demanding
a particular one buys a token test beside a real example.

What the policy does not ask of an item is derived from that item's kind and shape and is
reported as an exemption with its reason. Nothing declares an exemption, and there is no
file in which to record one.

## Failure behaviour

`majordomus quality report` measures the crate and answers with a typed report: the counts,
and one finding per violation carrying a stable code, the rule, the file and line, the
symbol, why it matters and what to do. The codes are
`RUST_PUBLIC_MISSING_DOCS`, `RUST_PUBLIC_THIN_DOCS`, `RUST_PUBLIC_MISSING_EXAMPLE`,
`RUST_EXAMPLE_NOT_EXECUTABLE`, `RUST_EXAMPLE_PLACEHOLDER`,
`RUST_EXAMPLE_DOES_NOT_NAME_SUBJECT`, `RUST_MODULE_MISSING_DOCS`,
`RUST_MODULE_MISSING_EXAMPLE` and `RUST_MODULE_MISSING_BEHAVIOURAL_TEST`. The command exits
`10` when any of them stands, which is the code every other unmet contract in this
executable uses, and the JSON and the human rendering are two readings of one value, so
they cannot disagree about the outcome.

The measurement is a capability, so it is answered identically on the command line, over
HTTP, over MCP and in the Cockpit. The `rust-quality` gate in `.ai/repo/ci/gates.yaml` runs
it, and `scripts/rust-check` runs it in the same order a person does.

Enforcement is a ratchet while the debt shrinks. `.ai/repo/rust-quality-baseline.txt`
records the findings that stood when this rule landed; the gate fails for a finding that is
not in it, so the debt can shrink and cannot grow. That is the mechanism
`project.rust-command-tested-in-file` describes and used, for the same reason: a blocking
rule with no migration path is a rule that gets reverted. The baseline is written only by
`--write-baseline`, which is a deliberate act, and the rule is not finished until the file
is empty and removed — an empty exemption list invites somebody to add a line to it.

The validator does not live in `lib/`. That library belongs to the shipped tool and a
project rule that needs a check gets a gate, not a doctrine — `project.rust-command-tested-in-file`
makes the argument and this rule follows it. Where there is no Rust crate the check is
skipped and says so.

## Verification

`cargo test --manifest-path apps/majordomus-cli/Cargo.toml quality`, which measures fixture
crates built for the purpose: a documented, exampled and tested crate has no findings, and
one crate per way of failing produces exactly its own code and nothing else — no example,
an ignored example, a placeholder example, an example about something else, a module with
no test. The validator's own negative fixtures are what keep it from becoming ceremonial.

`apps/majordomus-cli/tests/quality.rs`, which runs the measurement against this crate
through the built registry and the real command tree, and asserts the ratchet: no finding
outside the baseline.

`test/cases/96_rust_public_api_quality.sh`, which builds the executable, measures a fixture
repository through it, and asserts the exit code and the JSON of both a clean and a failing
tree.
{% endraw %}
