+++
title = "External interfaces are projections of one capability definition"
description = "External interfaces are projections of one capability definition"
weight = 91
[extra]
kind = "rule"
slug = "project-interfaces-are-projections-1"
identity = "project.interfaces-are-projections@1"
status = "active"
source = ".ai/repo/rules/project/interfaces-are-projections.v1.md"
+++
{% raw %}

## Rationale

The Rust executable exposes the same things through several interfaces. A tool table in the MCP code, a route table in the HTTP code, a hand-written OpenAPI document and a Markdown inventory would be four copies of one fact, and copies drift the moment one is edited. The registry in `apps/majordomus-cli/src/capability/` is the one place a capability exists; `docs/CAPABILITIES.md` says what is canonical, what is derived, and what is not authoritative.

## Required behaviour

An executable capability is one typed descriptor with one handler, composed into the registry in `capability/builtin.rs`. A declarative object is a file the layer's `sources.yaml` maps to a kind whose reading `share/kinds.yaml` and `share/schemas/` declare, read at run time; a repository adds kinds and schemas under `.ai/repo/knowledge/` and never edits Rust for another object of a known kind. MCP tools and resources, HTTP routes, the OpenAPI document, the Swagger UI shell, the `capabilities` commands, `docs/generated/` and the website's `/registry/` pages are projections built from the registry when asked; none of them declares a name, a description, a schema or a route of its own. The shell tool's allow-lists under `share/allow/` are generated from the schemas.

## Failure behaviour

A reviewer decides this rule for a change that adds a definition to a projection; the crate's suites decide the machine-checkable part: `tests/projections.rs` fails when a projection carries an entry the registry does not, or lacks one it declares, and `majordomus generate --check` fails when a committed projection differs from the registry.

The command line is the one interface that is declared twice — clap owns parsing, `--help` and value sets, so `CliExposure` is a claim about a declaration that lives elsewhere rather than a projection built by walking the registry. The two are compared: `capability/closure.rs` fails a capability that claims a command line the clap declaration does not answer (`CLOSURE_CLI_ABSENT`) or answers only as a group that cannot be run (`CLOSURE_CLI_NOT_RUNNABLE`), in `tests/projections.rs`, in the crate's own tests, and as the `projection` line of `capabilities validate`, which exits 10. A runnable command that no capability claims is reported rather than refused — the process commands and the lifecycle verbs are legitimately hand-written, and the count is the measure of how much of the command line is still not derived — but it is never silent. Each such command says why it is local, once, in `apps/majordomus-cli/src/cli/local.rs`, with one of four structural reasons and a note about that command in particular; `quality::parity` checks that statement in both directions, `capabilities projections` carries the commands that carry none, and `scripts/ci/projection-check` exits 10 on the first of them. There is no inventory of such commands anywhere else: a list of names without reasons is a third copy of a set two declarations already pin, and the one this repository kept went stale twice before it was removed.

## Verification

Review, `cargo test --manifest-path apps/majordomus-cli/Cargo.toml`, `majordomus capabilities validate` (the `projection` line) and `majordomus capabilities projections --unmet`, and `test/cases/76_capabilities_projections.sh`, which builds the executable, changes nothing but data, and reads the same capability back through every interface. The website is one more interface: its pages about the executable are rendered from the registry dataset and routed from the registry manifest (`test/cases/95_executable_reference.sh`), and `scripts/site-check` refuses a template, generator or navigation file that names a capability by hand.
{% endraw %}
