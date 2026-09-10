---
id: project.the-version-is-measured
version: 1
kind: rule
title: The version is measured against the public surface, not claimed
description: What a release's version number says about compatibility is decided by comparing the capability registry of the last release with this tree — an atom of the surface that is gone is a breaking change, one that is new is an addition — and a version smaller than what moved is refused. Below 1.0.0 the floor is a minor release and a removal is named rather than refused; at 1.0.0 the shift ends.
statement: A version number is derived from the difference between the public surface of the last release and this tree, never from the words in the commit messages; a declared version smaller than the surface change requires is refused, and a removal is named as a breaking change in the release that carries it whatever the number.
status: active
class: blocking
depends_on: [project.land-and-publish@1, project.interfaces-are-projections@1]
tags: [release, versioning, contract, evidence]
---

# Rationale

`project.land-and-publish` already says that a merge changing the public contract carries
its version, and it says so because the published site once named a version three releases
behind the code. What it could not say was *which* version, because the bump was decided by
the words in the commit messages: `feat:` meant minor, `fix:` meant patch, and a capability
deleted under a `refactor:` heading meant nothing at all.

A number a person chooses is a claim, and a claim about compatibility is the kind nobody can
check by reading. This repository shipped one: between `v0.3.1` and `0.4.0` the atom
`command scope classify` left the public surface and no release said so.

What left was a claim rather than a dispatch, and that is what made the silence expensive.
`repository.scope_classify` declared `cli: ["scope", "classify"]` so that its command module
could find its own id by path; the clap tree never had that subcommand, at `v0.3.1` or now,
and `majordomus scope classify <path>` parses `classify` as a path to judge and answers `out
undeclared classify [absent]` with exit 0. The generated reference, the OpenAPI document and
the published site advertised the command anyway, because each is a projection of the
registry — so a caller read it there, typed it, and got a plausible wrong answer rather than
a refusal. Deleting the claim was deliberate and correct (ADR 0027,
`project.commands-are-projections`). Every gate was green throughout, because until this rule
none of them looked at the surface.

Elm's package manager answers this by refusing to accept a version number a human chose: it
diffs the public API and computes the magnitude, and a package whose number is smaller than
its change cannot be published. The insight is not the diff, it is the refusal — a version
is worth something only when it cannot be smaller than the truth.

This repository can do the same without a new engine, because it already commits a
description of its whole public surface at every commit. The capability registry is one
declaration of which MCP, HTTP, OpenAPI and the command line are projections (ADR 0002,
ADR 0027), and `docs/generated/registry.json` is in the tree at every commit and at every
tag. The surface of any two refs can therefore be compared without building either of them.

# Required behaviour

The surface is the set of atoms a caller can hold: a public capability's identity, its
kind, the MCP tool and resource it answers to, the HTTP method and path it is bound to, and
the command-line path that dispatches it. Losing any one of them is a breaking change even
when the capability survives, because a caller holding the old one is broken either way.

Between the newest version tag and the tree:

- an atom that is **gone** implies a **major** change,
- an atom that is **new** implies a **minor** one,
- an unchanged surface implies **patch**, and owes no bump at all: this is judged over every
  tree, not only over a release, and most commits are behind the boundary.

The declared version — the one both writers state, which `majordomus release version` reads
— may not represent a smaller bump than the surface requires.

**Below 1.0.0 the required bump is a minor for any surface change, gone or new.** Semantic
versioning grants `0.y.z` a blanket exemption and Elm refuses that exemption by starting
every package at 1.0.0; this project is 0.x and has earned neither answer, so it takes the
strongest signal 0.x has instead of demanding 1.0.0 for one removal. **When the major
reaches 1, the shift ends** and the implied bump is the required one.

**A removal is named whatever the verdict is.** Below 1.0.0 it does not refuse the release,
and it is still stated as a breaking change: a caller who held what is gone otherwise finds
out by breaking.

Where it is stated is decided by what can hold it. The changelog is derived and the release
record is evidence written by the pipeline, so neither is authored — the one authored input
either of them reads about a change is the commit message. A removal is therefore named in
the subject of a commit inside the release that carries it, marked `type(scope)!:` or with a
`BREAKING CHANGE:` trailer, which `release/commits.rs` reads and `release/changelog.rs`
renders with a leading `**BREAKING**`. `majordomus release changelog` shows it under
`## Unreleased` at once; `docs/generated/changelog.*` shows it when the version ships,
because the committed artifact carries only published releases. The mechanism, and the field
the release record does not have, are in `docs/RELEASE.md` under *Where a removal is written
down*.

# Failure behaviour

`scripts/ci/version-matches-surface`, registered as the `version-surface` gate, exits 10
naming the atoms that moved, the bump each side represents, and the command that raises the
version; 12 when there is no release to compare with or a ref carries no registry. The
refusal is not a request to inflate the number: a removal that was a mistake is reverted,
and one that was deliberate is released as what it is.

# Verification

`test/cases/112_version_matches_surface.sh` builds the trees the gate must tell apart in
fixtures of its own — an unchanged surface, an addition under a patch, a removal under a
patch, a removal below 1.0.0, the same above it, and a repository with nothing to compare
with — so that the gate is measured by what it decides rather than by what this repository
happens to contain.
