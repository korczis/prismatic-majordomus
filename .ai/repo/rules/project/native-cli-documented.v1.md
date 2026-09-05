---
id: project.native-cli-documented
version: 1
kind: rule
title: A native command line command exists only with its documentation and its executable examples
description: Every command of the Rust executable is declared once in clap with the examples beside it, every command a person can run carries at least one example that the crate's tests execute against the built binary, and the generated reference, the site dataset and the routes under /docs/cli/ are projections that CI refuses to let go stale.
statement: Add or change a command of the Rust executable only by changing its clap declaration and the typed examples beside it and regenerating the projections; never by editing a generated reference, a site dataset, a route list or a template, and never by adding a runnable command that carries no executable example.
status: active
class: blocking
depends_on: [project.rust-canonical-declaration@1, project.derived-files-regenerated@1, project.no-claim-without-test@1]
tags: [rust, cli, documentation, projections]
---

# Rationale

A command line is the surface most people meet first, and documentation of it rots in a
particular way: the structure stays roughly right because `--help` is generated, while the
examples — the part a reader actually copies — quietly stop working. The examples are the
half no compiler checks, so they are the half that must be executed. `project.rust-canonical-declaration`
already says an operation of the executable exists in one declaration; this rule says the
same of the command line's documentation, and adds the part that declaration alone cannot
give: evidence that what the page shows still runs.

# Required behaviour

The clap declaration in `apps/majordomus-cli/src/cli.rs` is the structure: every command,
argument, default and accepted value. The typed metadata beside it in the same file
(`cli::EXAMPLES`) is the examples: an id, a title, a description, the argument vector, what
must be prepared first, and what the run must show. The command line a reader copies is
rendered from the argument vector and is never written out a second time.

Every command a person can run carries at least one example. Whether a command can be run is
read from clap, never from a list: a command with nothing under it, or one whose subcommand
is optional. A command that only groups other commands carries none.

`cli::validate` is the contract, in one place, reporting every violation of one run: a
command with no summary, an argument with no help, an enumerated value that says nothing, a
duplicate example id, an example the parser does not accept, an example on a command it does
not run, an example set for a command that does not exist, and a runnable command with no
example. Each violation names the command, the file to edit and the rule it broke.

Every projection is derived from that one tree and is regenerated with the change that moved
it: `docs/generated/cli.md`, `docs/generated/cli.json`, the `cli` of
`site/data/registry/registry.json`, `site/data/generated/cli.json`, and one route per command
under `/docs/cli/`. The route of a command is computed once, in the executable, and carried
in the data; no consumer derives one of its own. No list of commands, options, examples or
routes is maintained anywhere else — not in a template, not in the navigation, not in a
document, not in a test.

The *shell* tool `bin/majordomus` is a different program. `share/commands.yaml` and
`docs/CLI.md` are its, and neither is canonical for the Rust executable; the two command
surfaces are documented apart and their routes do not overlap.

# Failure behaviour

`majordomus capabilities validate` prints the violations and exits 10;
`apps/majordomus-cli/tests/cli_docs.rs` fails the crate's suite on the same contract, and
`tests/cli_examples.rs` fails when a documented example stops parsing, stops running, stops
answering what it is documented to answer, or leaves a process behind. `majordomus generate
--check` and `scripts/derive-check` fail on any stale projection. `scripts/site-check`
fails on a command with no route, a route with no command, a page that does not render its
own arguments or example anchors, a missing link up or down, and a command route typed by
hand into a template, the generator or the navigation. `scripts/site-probe` fails when a
declared route does not answer, or when a command page does not show what its data says it
shows. A reviewer refuses a command added without an example, and an edit to a generated
CLI reference.

# Verification

`apps/majordomus-cli/tests/cli_docs.rs` and `apps/majordomus-cli/tests/cli_examples.rs`;
`majordomus capabilities validate`; `just derive` and `just derive-check`;
`scripts/site-check` and `scripts/site-probe` over the built site.
