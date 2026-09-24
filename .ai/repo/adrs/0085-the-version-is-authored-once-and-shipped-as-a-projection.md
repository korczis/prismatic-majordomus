---
schema: adr/v1
id: adr-0085
kind: adr
title: The version is authored once, in the crate manifest, and shipped to the shell tool as a generated projection
status: proposed
date: 2026-09-24
tags:
  - release
  - versioning
  - projections
  - distribution
provenance:
  origin: authored
related:
  - rule:project.release-is-a-projection
  - rule:project.the-version-is-measured
  - rule:project.distribution-canonical
  - rule:project.generated-artifacts-are-typed
  - file:apps/majordomus-cli/src/release/version.rs
  - file:apps/majordomus-cli/src/generate.rs
  - file:bin/majordomus
  - file:scripts/release-version
  - file:scripts/ci/release-check
  - test:test/cases/484_the_version_is_authored_once.sh
  - test:test/cases/103_release_projection.sh
---

# 85. The version is authored once, in the crate manifest, and shipped to the shell tool as a generated projection

> **Status: proposed.** Acceptance is the maintainer's act. This record amends ADR 0029, ADR
> 0019 and ADR 0051 in the one respect each of them states the version twice; everything else
> in them stands.

## Context

The owner's requirement was plain: *make sure there is one place where versioning is
handled*. The repository did not have that. The product version was authored in two places:

```text
  apps/majordomus-cli/Cargo.toml   version = "0.8.0"       the crate's version
  bin/majordomus                   MJ_VERSION="0.8.0"      what the shell tool prints
```

ADR 0029 gave the pair one writer, `release bump`, and `scripts/release-version --check`
proved they agreed. It rejected a single source for four reasons (ADR 0029, *Collapsing the
version to a single source*):

- (a) an installed tree has no `Cargo.toml`, so the shell tool cannot read it at run time;
- (b) a build script writing `MJ_VERSION` into `bin/majordomus` would make a tracked file a
  build output;
- (c) the crate is compiled before the shell tool exists;
- (d) generating one from the other at run time is a run-time dependency between two
  programs that ship separately.

Each reason is correct about the alternative it names. None of them holds against a
*generated projection shipped in the distribution*, and a projection is how this repository
states every other derived fact.

## Decision

**The version is authored in exactly one place: `[package] version` in
`apps/majordomus-cli/Cargo.toml`.** Every other statement of it is derived from that line and
checked, or is a record of a version that was released:

| Statement | Class | Written by | Held by |
|---|---|---|---|
| `apps/majordomus-cli/Cargo.toml` `[package] version` | authored — the one place | `majordomus release bump` | `release analyze`, `version-surface` |
| `crate::VERSION` (clap, MCP, OpenAPI, health, ledger) | compiled | cargo | the crate's own tests |
| `share/version.txt` | generated projection | `majordomus generate` (target `distribution`) | `generate --check`, `release-version --check`, `release-verify` |
| `apps/majordomus-cli/Cargo.lock`, the crate's entry | derived by cargo's rule | `release bump`, alongside the manifest | `cargo build --locked`, `release-check` 2b |
| generator stamps, the changelog, the site's version | generated | `majordomus generate`, `scripts/derive` | `generate --check`, `derive-check` |
| `.ai/repo/releases/*.yaml`, `latest.json`, `RELEASE.json` | records of a released version | the release pipeline | `release-check`, the installer |
| `bin/majordomus` | **no statement** — it reads `share/version.txt` | — | case 484 |

**The shell tool reads a projection, from beside itself.** `share/version.txt` is a typed
`generate` artifact (`ArtifactFormat::Text`, indexed in `docs/generated/artifacts.json`,
`merge=derived`), generated under the same guard the design projection uses — the share and
the crate are both in this repository — so a managed repository never gets one. Its value is
the authority read directly (`release::version::declared`). `bin/majordomus` reads its
`version=` line at start-up with shell builtins alone, from `bin/../share/version.txt`, and
never from `MAJORDOMUS_SHARE`, which names data to read and not the identity of the program
reading it. A distribution without it exits 12 and names the file. Every archive already
ships `share/` whole, so the checkout, the installed tree, the submodule install and the
installer's staging directory read the version by one code path.

**One writer, writing one authored line.** `release bump` rewrites the manifest's version
line and keeps the lock's own `majordomus-cli` entry in step — not a second declaration but
cargo's record, kept by the writer because a lock left behind fails every `--locked` build,
including the launcher the git hooks and the MCP server use. It no longer writes
`bin/majordomus`. The projection and every stamp follow from `scripts/derive`.

**One reader of the authority per language.** `release::version::declared` in Rust (the
generate command's foreign-generation guard reads through it), `scripts/release-version` in
shell. Gates and cases ask them; none parses the manifest itself.

**What refuses a second statement is typed.** `release::version::diagnose`, in the release
module, reports:

- `version-stated-by-hand` (error) — a version-shaped carrier written by hand in `bin/`,
  `lib/`, `scripts/` or `share/` outside a generated artifact: a shell assignment to a
  `…VERSION` name, a `version:` or `"version":` member, or the product's `majordomus X.Y.Z`;
- `projection-stale` (warning) — the projection is behind the manifest or missing: the state
  every bump leaves until the derivation runs. A warning, because `generate --check` is what
  refuses it, and a writer that refused its own un-derived state could not correct a bump;
- `writers-disagree` (error) — the projection is ahead of the manifest or unrelated to it: a
  version no derivation writes.

`release analyze` carries these in its plan on every surface; `release version` prints them
and exits 10; the gate `version-authored-once` (structure job, `always: true`) runs
`bin/majordomus-cli release version`. The public field names of the version report and the
plan (`tool`, `agree`, `tool_version`, `writers_agree`) are kept and redefined as the
projection and whether it is current, so no caller's contract moves.

## Why the four reasons no longer hold

- **(a) no `Cargo.toml` when installed.** The projection ships in `share/`, beside the tool.
- **(b) a tracked file as build output.** It is a `generate` target, like the ~130 other
  projections `generate --check` holds, not a side effect of `cargo build`.
- **(c) the crate compiled first.** Order does not matter: `generate` refuses to run with an
  executable of another version (`refuse_foreign_generation`), and the projection's value is
  read from the manifest, not from the executable.
- **(d) a run-time dependency between programs.** There is none. The shell tool reads a text
  file its own distribution carries.

## Alternatives rejected

**Keep the literal and check it.** That is the state this replaces — the parallel hand-kept
statement the requirement forbids.

**A generated region inside `bin/majordomus`.** The artifact manifest indexes whole files by
hash, so half an executable cannot be a typed artifact; `.gitattributes` cannot mark half a
file `merge=derived`; and `generate` would write into `bin/`.

**Read `Cargo.toml` in a checkout and `RELEASE.json` when installed.** Two code paths for one
fact, a fork on the hook hot path, and no answer in a submodule install.

**Ask the executable (`libexec/majordomus-cli --version`).** A process per invocation, and
in a checkout the launcher may build first.

**Author the version in `share/version.txt` and compile it in with `include_str!`.** It would
leave a placeholder in `Cargo.toml` and `Cargo.lock`, and the deployment image copies only
the crate into its build stage.

**A repository-wide sweep for the current version string.** The design review measured it
against the next likely versions: it fails on doc tests, fixtures and a dependency's version
range that happen to share the number, and `grep -w` cannot see the `v`-prefixed form. The
typed diagnostic checks declared *shapes* where the tool's own files live, and
`generate --check` holds everything generated.

## Consequences

Raising the version is one edit to one file, made by one command, and derivation:

```sh
bin/majordomus-cli release bump      # the manifest's line, and the lock's own entry
scripts/derive                       # builds, then projects share/version.txt and every stamp
```

`scripts/release-version --check` now asks the question `generate --check` leaves open —
not whether the file is current, but whether the tool actually prints the authority — and the
release plan, `release-verify` on every archive and the installer's own check continue to
ask it of the shipped tool unchanged.

A distribution that lost `share/version.txt` stops at start-up for every command, hooks
included. That is intended: a tool that cannot say what it is has no business answering
anything else. Every fixture in the suite that copies `bin/` also copies `share/`, and the
`context` archive profile keeps the file although it drops other projections.

`package.json` and `package-lock.json` lost a stray `"version": "0.1.0"` of the private site
package, which nothing read and which contradicted the one place.

What holds it: `test/cases/484_the_version_is_authored_once.sh` — every section of it run
against the tree before this change and failing there — case 103 over the writer and the
report, case 112 over the measured bump, the unit and doc tests of `release/version.rs`,
`scripts/ci/release-check`, `scripts/release-version --check` and the gate
`version-authored-once`.
