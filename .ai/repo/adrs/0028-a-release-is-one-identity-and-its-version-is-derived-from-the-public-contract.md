---
schema: adr/v1
id: adr-0028
kind: adr
title: A release is one identity, and its version is derived from the public contract rather than declared
status: proposed
date: 2026-09-09
tags:
  - release
  - version
  - compatibility
  - projections
related:
  - rule:project.release-identity
  - rule:project.distribution-canonical
  - file:apps/majordomus-cli/src/release/mod.rs
  - file:docs/RELEASE.md
  - test:test/cases/104_release_identity.sh
provenance:
  origin: extracted
  derived_from:
    - file:apps/majordomus-cli/src/release/contract.rs
    - file:apps/majordomus-cli/src/release/diff.rs
    - file:scripts/release-version
---

# 28. A release is one identity, and its version is derived from the public contract rather than declared

## Context

Five things in this repository knew about versions, and none of them was the authority.

`apps/majordomus-cli/Cargo.toml` stated a version and the executable compiled it in.
`bin/majordomus` stated the same version again, as a shell literal, because an installed
tree has no crate manifest and the shell tool must be able to say what it is.
`scripts/release-version --check` reconciled the two — when somebody ran it. Downstream,
`scripts/generate-site-data` read the website's navigation-bar version from the *shell*
literal while `docs/generated/openapi.json` took its `info.version` from the *crate*, so
the site's header and its API reference were fed by two structurally different pipelines
that agreed only because the reconciliation had been run recently.

There was no changelog at all. A release's notes were whatever GitHub generated from
commit subjects, so the only durable record of what a release contained was a list of
commit messages, and a reader wanting to know whether an upgrade was safe had to read
them.

And nothing decided whether a version was *allowed*. A release that removed an HTTP route
could be tagged as a patch and every gate in the repository would pass. The number a
release carried was a number somebody typed.

The repository already had the parts of an answer. The capability registry is canonical and
already projects itself into `docs/generated/registry.json`; the command graph does the
same into `docs/generated/cli.json`; the document schemas are generated from `.proto`
files; the distribution model is canonical and generates the installer, the build matrix
and the public release metadata. Every one of those is committed, checked by
`majordomus generate --check`, and therefore readable at any past commit through git. What
was missing was the observation that, taken together, they *are* the public contract — and
that a release's compatibility is a diff between two commits of that document, not an
opinion.

## Decision

**One authority.** The crate's version is the only place a version is written. The shell
tool reads `share/version.txt`, a generated projection of it, which a release archive
carries in full; `scripts/release-version --check` remains as the mechanical proof. No
other file states a version, and the two site pipelines now both descend from the crate.

**The contract is a committed document.** `docs/generated/contract.json` is generated at
every commit from four inputs the process already has: the capability registry (each
capability's kind, its input and output schemas property by property, and every projection
it declares), the command graph (each runnable command of the executable and the shell
tool, with its arguments), the document schemas (field by field, because every
repository's own files are validated against them), and the distribution model's targets.
Everything is normalised — two spellings of an optional string produce one fact — sorted,
and fingerprinted.

**Compatibility is measured, not declared.** A release's impact is the diff between that
document and the same document at the last published stable release, read out of git.
Each fact's meaning is a row in one policy table: an input property that disappears is
breaking because the input refuses unknown keys; a projection that is withdrawn is breaking
because it is the address a caller holds; a capability that appears is additive. A fact
nobody classified reports as unclassified and costs nothing, so the table's gaps are
visible rather than silently generous.

**The version follows from the impact by a stated policy**, and the impact is kept distinct
from the bump so that an explanation can walk back along the chain. At and above 1.0.0 the
mapping is the specification's. Below it — which is where this project is, and where the
specification says nothing — a breaking change moves the minor and everything else moves
the patch, which is the rule Cargo already applies to a `0.x` dependency.

**A version below the minimum is refused**, by `majordomus release prepare` and by
`majordomus release check`, with no flag that lifts it.

**A change is written once.** A record under `.ai/repo/changes/` carries the half of a
release a diff cannot reach: what was fixed, why a break was worth making, and the way
through it. `CHANGELOG.md`, the release notes, the release manifests, the Cockpit's release
page, the API and MCP answers are projections of those records and of the diff.

**A release has one identity**, and the release manifest is where it is proved: the tag,
the version, the commit, the contract fingerprints on both sides, the changes, the required
migration documents and the artifacts with their digests, in one generated document per
release.

**Four versions, never collapsed.** What this tree would release, what is published, what
this process is and what a deployment reports are four facts. The Cockpit's existing
version display — which showed one number — became a link to the release page carrying the
state of that release, and says `drift` when a deployment reports something else.

## Consequences

Adding a capability, a command, a document field or a platform now moves the contract
snapshot, which moves the required version, which the release check enforces. Nobody has to
remember any of it, and nobody can decide otherwise for a particular release.

The cost is a generated document that must stay current. `docs/generated/contract.json` is
subject to the same `generate --check` the other twenty generated artifacts are, on the
pre-commit hook and in CI, so a stale snapshot is caught where every other staleness is —
but it is one more thing that can be stale, and a verdict computed over a stale contract
would be quietly wrong. The engine therefore checks the committed snapshot's capability
surface against the live registry on every read and says `CONTRACT_SNAPSHOT_STALE` when
they disagree.

The contract deliberately excludes the workflow runner's recipes. A `just` recipe is not
shipped and the runner is not installed everywhere, so including them would make the
document differ between two machines reading the same commit — which is the one thing a
baseline may never do.

The first release measured this way has nothing to measure against: `v0.3.1` was published
before the snapshot existed. That is a fact about history and not a failure, and it is
reported as an informational diagnostic rather than a blocking one — a gate that is red
forever is a gate everybody learns to ignore. Until the next release, the impact falls back
to what the unreleased change records claim, and every report says which of the two it used.

## Alternatives rejected

**Derive the bump from commit messages.** Conventional-commit prefixes are what most
projects use. They record what somebody *meant* to do; the contract records what they
*did*. A `feat:` that removes a field and a `chore:` that adds an endpoint are both
ordinary, and neither is visible to a parser of subject lines. Commit subjects remain
available as supplementary evidence on a change record and decide nothing.

**Take a `semver` crate.** The grammar is small, total and frozen, and this repository has
no YAML crate and no TOML crate for the same reason. What a crate would give is
correctness, and correctness here is given by property tests against the specification's
own ordering examples — including the case the previous five-tuple sort key got wrong,
where `1.0.0-rc.10` sorted below `1.0.0-rc.2`.

**Keep the two version writers and check them.** This was the state of the art here, and
the check worked. It is still one fact written twice, and the reconciliation is a step
somebody performs rather than a property the tree has. Generating the second copy costs one
artifact and removes the class.

**Make the contract snapshot a hash.** A single digest per release would answer "did
anything change" cheaply and answer nothing else. The whole value is in being able to say
*which* fact moved and what it costs, which is what makes a required bump explainable and
therefore trustworthy.

**Expose release preparation over HTTP or MCP for surface symmetry.** Nothing served on the
loopback socket writes to the repository, and a release is the last thing that should be
the exception. Reading the release state is exposed everywhere; preparing one is a command
of the executable, and publishing one is the pipeline's.

**Rebuild the contract on every read.** The engine reads the committed projection instead.
A full snapshot needs the command graph, which costs a subprocess, and the Cockpit renders
a version indicator on every page. Reading the committed document also means the current
contract and the baseline contract are the same kind of document, produced by the same code
path.
