---
id: project.the-version-is-measured
version: 2
kind: rule
title: The version is measured against the public surface, not claimed
description: What a release's version number says about compatibility is decided by one typed engine comparing the capability registry of the last release with this tree — identities, bindings and both schemas of each capability — and a version smaller than what moved is refused, by the writer as well as by the gate. Commit subjects are evidence beside that verdict and never the authority for it. Below 1.0.0 the floor is a minor release and a removal is named rather than refused; at 1.0.0 the shift ends.
statement: Measure the compatibility level from the public contract with the one engine every surface renders, and let one writer apply it; a commit label, a changelog heading or an explicit version that understates what the contract did is a claim nobody checked, and neither the writer nor any override may go under the measured minimum.
status: active
class: blocking
depends_on: [project.land-and-publish@1, project.interfaces-are-projections@1, project.release-is-a-projection@1]
tags: [release, versioning, contract, evidence]

x-majordomus:
  tests: [test/cases/112_version_matches_surface.sh, scripts/ci/version-matches-surface]
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

## What version 1 of this rule left open

Version 1 was enforced by a shell script, and three things followed from that which only
showed up in use (ADR 0051).

**It was not the only answer.** `majordomus release bump` — the one writer, which is what
actually sets the number — computed its own level from the conventional-commit types, so the
repository had a measurement and an inference answering the same question. They disagreed
exactly on the change that removes something, and the inference won, because the writer runs
on a developer's machine and the gate ran afterwards, if at all.

**It ran almost never.** The gate was declared in the CI model and selected by no path class,
so it was planned only on a full run. A pull request that added nine capabilities and left
the version alone was never asked the question; `origin/master` was carrying exactly that
when this version was written.

**It could only see names.** It compared sorted lines, so a required input field added, an
output field removed, an enum narrowed or a type changed — all of which break a caller, and
all of which the committed registry describes in full — were invisible to it.

# Required behaviour

**One engine measures, and nothing else decides.** `apps/majordomus-cli/src/release/compat.rs`
computes the level from the public surface (`release/surface.rs`), and every surface that
shows a version verdict renders that one value: the command line, `GET /api/v1/release/analysis`,
the `majordomus_release_analysis` MCP tool, the Cockpit, the generated documents, the CI gate
and the writer. There is no second computation anywhere.

The surface is what a caller can hold: a public capability's identity and kind, the MCP tool
and resource it answers to, the HTTP method and path it is bound to, the command-line path
that dispatches it, **and both of its schemas**. Losing any one is breaking even when the
capability survives, because a caller holding the old one is broken either way.

Between the last release and the tree:

- an atom that is **gone**, or a contract that narrowed under a caller, implies a **major**
  change,
- an atom that is **new**, or a contract that widened, implies a **minor** one,
- an unchanged surface owes **no bump at all**: this is judged over every tree, not only over
  a release, and most commits are behind the boundary. A gate that demanded a bump for each
  of them would be a thing to work around within a day.

**Schemas are compared as contracts, in both directions.** An input is what a caller sends,
so accepting more is compatible and demanding more is breaking; an output is what a caller
receives, so promising more is compatible and promising less is breaking. The same edit is
minor in one and major in the other. Prose — descriptions, titles, examples — is normalised
away first, so a reworded doc comment is not a release. A contract mutation the comparator
does not model is counted **breaking** and says so: a false major is an argument someone can
win, a false patch is a caller who finds out by breaking.

**This tree is read live; a release is read from its commit.** The current surface comes from
the in-process registry, so a stale committed projection cannot make the comparison describe
an older tree. A released ref has no such option, so the registry committed at it is used —
and a ref that carries none **refuses**, because an empty baseline reports every capability
as new and an empty head reports every one as removed.

**The baseline is the newest release the layer records**, not the newest thing that looks
like a tag; a tag without a record and a record without a tag are both reported.

**Conventional commits are evidence.** They stay the changelog's prose and the human account
of a change, they are carried in the plan beside the verdict, and when they classify a window
as less than the contract moved by, the answer says so rather than deferring to them.

**One writer, and no override beneath the floor.** `majordomus release bump` reads the plan
and applies it — `plan → validate → apply(plan)`, never a second computation. `--level` and
`--exact` may name a version **above** the measured minimum, reported with its provenance,
and are refused below it with nothing written. A version never goes down.

**Below 1.0.0 the required bump is a minor for any surface change, gone or new.** Semantic
versioning grants `0.y.z` a blanket exemption and Elm refuses that exemption by starting
every package at 1.0.0; this project is 0.x and has earned neither answer, so it takes the
strongest signal 0.x has instead of demanding 1.0.0 for one removal. **When the major
reaches 1, the shift ends** and the implied bump is the required one.

This is a typed, versioned policy — `majordomus/version-policy/v1`, with a mode decided by
the baseline version alone — carried into every verdict so that a decision recorded today
stays explicable when the policy changes. It is not a special case inside a script.

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

`scripts/ci/version-matches-surface`, registered as the `version-surface` gate, is an adapter
over `majordomus release analyze` and holds no semantics of its own. It exits 10 naming every
movement and the reason each counts for what it does, and 12 when the analysis cannot be made
at all — no release to compare with, a baseline whose registry is not committed, a shallow
clone. Those two are kept apart deliberately: "I could not tell" and "the answer is no" are
different facts, and a check that reports the first as the second has gone blind without
saying so.

The gate is selected by the `rust`, `rust-generated` and `distribution` path classes, so a
change that can move the public contract runs it **before it lands**; it runs in the `rust`
job, which has the executable and the full history it needs.

`majordomus release bump` refuses, writing nothing, when the version asked for is below the
measured floor, below what the tree already declares, or when the plan itself carries an
error diagnostic.

The refusal is not a request to inflate the number: a removal that was a mistake is reverted,
and one that was deliberate is released as what it is.

# Verification

`apps/majordomus-cli/src/release/compat/tests.rs` carries the compatibility matrix: every row
above, both policy modes, the schema comparison in both directions, and the determinism
invariants — reordering is not a change, a surface diffed with itself is empty, and the report
is a function of the two surfaces rather than of the order the registry enumerated them in.
`release/surface.rs` carries the fingerprint, the normalisation and every way a baseline can
be unreadable.

`test/cases/112_version_matches_surface.sh` is the behavioural half, in a disposable
repository with a real history and real published releases: an additive change refused and
then fixed by the writer, an undershooting override refused with both version sites
untouched, an overshooting one allowed and reported as an override, an unchanged contract
owing nothing, a removal named as breaking, an unreadable baseline refused as unreadable
rather than as an empty diff, a tag nobody recorded reported, and — the assertion that keeps
the adapter an adapter — the gate and the engine exiting with the same code on the same tree.
