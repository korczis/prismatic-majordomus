+++
title = "Every command of the Rust executable's command line carries a summary, a long description, help on every argument and at least one example, and every example shown to a reader is executed against the built executable"
description = "A command of the native Rust command line cannot exist quietly. Whatever --help lists, the reference under /docs/cli/ lists too, with the same words, because both are projections of one declaration in apps/majordomus-cli/src/cli.rs. A command that runs on its own — as opposed to a parent that only groups the commands under it — must carry at least one example, and that example is not prose: it is an argument vector with a title, a description, whatever must have run before it, and a typed statement of what running it must produce. The command line a reader sees on the page is composed from that vector, so there is no second spelling of it to fall out of date."
weight = 140
[extra]
claim_id = "cli-documentation-executable"
status = "guaranteed"
source = "docs/claims/cli-documentation-executable.md"
+++
{% raw %}

## What it means

A command of the native Rust command line cannot exist quietly. Whatever `--help` lists, the reference under `/docs/cli/` lists too, with the same words, because both are projections of one declaration in `apps/majordomus-cli/src/cli.rs`. A command that runs on its own — as opposed to a parent that only groups the commands under it — must carry at least one example, and that example is not prose: it is an argument vector with a title, a description, whatever must have run before it, and a typed statement of what running it must produce. The command line a reader sees on the page is composed from that vector, so there is no second spelling of it to fall out of date.

The examples are then run. `apps/majordomus-cli/tests/cli_docs.rs` takes the same metadata the page renders and executes it against the compiled binary in a disposable repository, one per example — including the two long-running commands, which are spawned, probed and stopped. Where an example is deliberately not executed, it says so and says why, in its own words, on its own page. So "documented" and "tested" are the same object rather than two that happen to agree.

## How it works

`cli::tree()` walks the built clap `Command` and joins each node with the `EXAMPLES` const declared beside that command's arguments; the result, `CommandDoc`, carries the path, the route, the usage line, every argument with its default and accepted values, and every example. `cli::validate_docs` reports, in one pass, every command without a summary or a long description, every argument without help, every command that runs on its own and carries no example, every duplicate example id, every example attached to the wrong command, and every example the canonical parser does not answer the way the example says — each violation naming the command, the file to edit and the rule broken. `majordomus capabilities validate` runs it and exits 10, which puts it in `scripts/rust-check` and in the `rust` job of CI without a gate of its own.

The tests are two levels over the same data. Level A parses every documented argument vector, and every vector an example says must run first, with `Cli::try_parse_from`: a renamed flag, a removed enum value or a missing required argument makes the example stale immediately. Level B runs each example against the built executable in a fresh fixture repository — asserting the exit code, the output, the JSON document or the JSON pointer the expectation names; `majordomus serve --port 0` is spawned, the address it logs is read within a deadline, its documented route is fetched over a real socket, and it is stopped by closing stdin; `majordomus mcp --standalone` is spoken to with a real `initialize` request. Both are held by a guard that kills the child whatever the probe found, so no test can leave a server behind.

Downstream, the same tree becomes `docs/generated/cli.md`, the `cli` of `docs/generated/registry.json`, the `cli` and `cli_pages` of `site/data/registry/registry.json`, and one page per command under `/docs/cli/`. Each command's route is derived once, in Rust, so nothing else slugifies a command path; `scripts/site-check` compares the routes the build produced with the routes the executable declares, as sets and in both directions, and `test/cases/97_cli_reference.sh` proves the projection by editing the tree and watching pages appear and disappear.

## How to see it

```bash
majordomus capabilities validate | grep '^OK   cli'
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test cli_docs
just test-shell 97_cli_reference
scripts/site-build && scripts/site-check | grep '^OK   cli'
```

## What it does not cover

It is about the *native* command line, the Rust executable's. The shell tool's task lifecycle is a different program with its own catalogue (`share/commands.yaml`), its own narrative (`docs/CLI.md`) and its own pages; neither is generated from the other. The contract requires an example, not a good one: a reviewer still decides whether an example teaches anything. One example is deliberately not executed — recording a benchmark baseline replaces the repository's accepted evidence and takes a full measurement run — and it carries that reason wherever it appears, so the page never claims evidence it does not have.

## Why it exists

Documentation that nobody runs decays silently, and a command that ships with a one-line summary is a command a reader cannot use. The two failures have the same shape as the drift `project.no-claim-without-test` exists to prevent, so they get the same answer: put the documentation where the declaration is, make the example the thing the test runs, and let CI refuse the rest. The intended outcome is that adding an undocumented command is harder than adding a documented one.
{% endraw %}
