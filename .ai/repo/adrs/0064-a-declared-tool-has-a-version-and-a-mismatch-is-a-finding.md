---
schema: adr/v1
id: adr-0064
kind: adr
title: A declared tool has a version, and a mismatch is a finding
status: proposed
date: 2026-09-14
tags:
  - environment
provenance:
  origin: authored
---

# 64. A declared tool has a version, and a mismatch is a finding

## Context

`catharsis-as-a-service` pins the generator that builds it: `.zola-version` contains `0.23.6`, and
`scripts/validate.sh` refuses to run any other version — deliberately, because a static site generator's
output changes between patch releases and the repository's HTML validation is exact. On 2026-09-14 the
machine had 0.23.4, Homebrew offered 0.23.5, and the gate stopped at its first step. Every other check in
that repository was unreachable until the pin was satisfied.

The recipe that satisfies it exists and is exact: `.github/actions/setup/action.yml` downloads
`zola-v${version}-x86_64-unknown-linux-gnu.tar.gz` from the release page and checks it against a recorded
SHA-256. That knowledge is reachable by CI and by nobody else. The session satisfied the pin by reading
the workflow, translating it to the local architecture and downloading the tarball by hand into a scratch
directory — a private act that left no trace, taught nothing to the next worker, and is the reason this
decision exists.

Majordomus already models declared toolchains: `apps/majordomus-cli/src/environment/toolchain.rs` detects
rust, node, just, python, go, elixir and deno from the markers those ecosystems use, and types each as
`{declared, declared_by, installed, availability}`. Two things it cannot express:

- `ToolchainAvailability` is `Installed | Missing | Unknown`. A repository that declares node 22 on a
  machine running node 18 reports `Installed`. There is no mismatch state, so there is no mismatch
  finding, and the one diagnostic that exists fires only when the binary is absent entirely.
- The detector table is Rust. A repository that pins a binary the table does not know — zola, terraform,
  a house tool — has no way to declare it at all. `majordomus env` cannot see the pin that stops the
  build.

## Decision

**A declared version that differs from the installed one is its own state.** `ToolchainAvailability`
gains `Mismatch { declared, installed }`, and `doctor` reports it as a finding, with the file that
declared it and the version that answered.

**A repository may declare a tool this tool has never heard of.** The declaration is data in the
repository's own layer: the binary, the file that carries the version, how to read it, and the probe that
asks the installed binary what it is (`zola --version`, field 2). Detection stops being a Rust table that
must grow for every ecosystem and becomes a table this tool ships *plus* what the repository adds.

**A declaration may carry the recipe that satisfies it, and Majordomus never runs it.** A `how` field —
the URL pattern, the digest, the command — is printed by the finding and executed by a person. This ends
the failure that forced the decision (the recipe locked inside a CI action) without making a supervisory
control layer into a package manager, which would need the network, a trust model and a place to put
binaries, none of which it has or should have.

**A mismatch is advisory, and only the repository's own gate may make it fatal.** A pin sometimes
deliberately lags a machine; the tool reports, the gate refuses. This is the same division as everywhere
else: Majordomus says what is true, the repository decides what is fatal.

## Alternatives rejected

**Install the declared toolchain.** It is the obvious next step and it is a different product. Downloading
and executing binaries on a developer's machine requires a trust model this tool has deliberately avoided
(`project.no-network-no-eval`), and every ecosystem already has a version manager that does it better.

**Read the CI workflow and learn the pin from there.** A workflow is a projection of what the repository
requires, not the statement of it, and this repository's own doctrine is that a semantic definition
repeated across projections is a design defect (ADR 0004). The declaration belongs in the layer; CI reads
it too.

**Treat a mismatch as `Missing`.** It reads as a lie in the one place a person looks when the gate stops:
the binary is there, on the PATH, answering. A diagnostic that misdescribes what it found is worse than
no diagnostic.

## Consequences

`majordomus env` and `doctor` gain a finding class that will fire on machines that have been working fine,
which is the intended effect and will be noisy once.

The toolchain detector becomes partly data, which means a schema for it, and a repository that writes a
bad probe gets a bad answer — bounded by the fact that the probe is a read-only command the repository
chose to run against its own binary.

The knowledge of how to satisfy a pin moves out of CI and into the layer, where the next worker — human or
not — finds it by asking the tool that just told them the version is wrong.
