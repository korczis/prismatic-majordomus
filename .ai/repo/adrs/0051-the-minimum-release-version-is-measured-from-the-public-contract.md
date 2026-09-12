---
schema: adr/v1
id: adr-0051
kind: adr
title: The minimum release version is measured from the public contract, and conventional commits are demoted to evidence
status: proposed
date: 2026-09-12
tags:
  - release
  - versioning
  - compatibility
  - projections
related:
  - rule:project.the-version-is-measured
  - rule:project.release-is-a-projection
  - rule:project.interfaces-are-projections
  - file:apps/majordomus-cli/src/release/surface.rs
  - file:apps/majordomus-cli/src/release/compat.rs
  - file:scripts/ci/version-matches-surface
  - file:docs/RELEASE.md
provenance:
  origin: extracted
  derived_from:
    - file:apps/majordomus-cli/src/release/compat.rs
    - file:apps/majordomus-cli/src/release/surface.rs
    - file:apps/majordomus-cli/src/commands/release.rs
---

# 51. The minimum release version is measured from the public contract, and conventional commits are demoted to evidence

> **Status: proposed.** This record was extracted from the implementation, and
> `lib/adr.sh` refuses an extracted record that calls itself accepted: acceptance is the
> maintainer's act, and a tool that writes it turns its own inference into repository truth.
> The subsystem below is implemented, gated and tested; whether the *decision* stands is not
> something this record may assert about itself.

## Context

ADR 0029 gave the version one *writer*, which it had never had, and that half of it stands:
the version is stated in two files for a real reason, `majordomus release bump` is the only
thing that writes them, and `scripts/release-version --check` proves the work of one command
rather than the memory of one person.

It did not give the version an *authority*, and it said so. Its own consequences section
names the gap as an accepted cost:

> And the bump's default is an inference from commit subjects, so a breaking change
> committed as `feat` produces a minor bump; the override exists for that […]

That cost turned out to be the whole problem, for three reasons that were not visible when
it was written.

**There were two answers to one question, and the weaker one won.** After ADR 0029 landed, a
second mechanism arrived: `scripts/ci/version-matches-surface`, which compared the capability
registry of two refs and refused a tree whose version was smaller than the surface implied.
So the repository had a measurement *and* an inference, they answered the same question, and
they disagreed exactly where it mattered — on the commit that removes something. The writer
won every time, because the writer runs when a person raises the version and the gate ran
afterwards, if at all.

**The gate was in no path class.** It was declared in `.ai/repo/ci/gates.yaml` and no class
selected it, so it ran only on a full plan — a master push, the schedule, a dispatch. A pull
request that added nine capabilities and left the version alone was never asked the question.
`origin/master` at `90a875cf2` was, in fact, carrying nine added public capabilities and
still declaring `0.5.0` when this was written; the gate had been red on master and its
verdict was reaching nobody before the merge.

**The measurement was a shell script, so it could only see names.** It compared sorted lines:
a capability's identity, its kind, its MCP tool and resource, its route, its command path. It
could not compare the two schemas — which the committed registry carries in full — so a
required input field added, an output field removed, an enum narrowed or a type changed were
all invisible to it, and all of them break a caller. Its `0.x` policy was a shell function
nothing could test, and its baseline was `git tag | sort -V | tail -1`, which is a different
question from the one `release version` asked (the newest release *record*) — two baselines
that happened to agree and were not required to.

## Decision

**The public contract decides the minimum version, and one typed engine measures it.**

- **The surface is the registry.** Every interface here is a projection of the capability
  registry (ADR 0002, ADR 0027), so the registry is the public contract and the projections
  are what it looks like from outside. Nothing crawls Swagger, parses generated Markdown or
  calls a running server to find out what the contract is: comparing projections compares
  renderings, and comparing the registry compares the promise.
- **This tree's side is read live; a release's side is read from its commit.** The current
  surface comes from the in-process registry, so a stale `docs/generated/registry.json`
  cannot make the comparison describe an older tree. A released ref has no such option — its
  executable is not available — so the registry committed at it is used, which is why that
  file is committed at every commit at all. A ref that carries none refuses; it never
  compares against an empty surface, because an empty baseline reports every capability as
  new and an empty head reports every capability as removed.
- **Schemas are compared as contracts, in both directions.** An input is what a caller sends,
  so accepting more is compatible and demanding more is breaking; an output is what a caller
  receives, so promising more is compatible and promising less is breaking. The same textual
  edit is minor in one and major in the other, which is the mistake a diff of two schemas
  makes. Prose — descriptions, titles, examples — is normalised away first, so a reworded doc
  comment is not a release.
- **An unmodelled contract mutation is breaking.** The comparator names the mutations it
  models and refuses to guess at the rest: a constraint key it has no rule for, changed, is
  major with a diagnostic saying so. A false major is an argument someone can win; a false
  patch is a caller who finds out by breaking.
- **The `0.x` policy is a typed, versioned declaration.** Below `1.0.0` an unchanged surface
  owes nothing and any movement of it is at least a minor — so a breaking change costs a
  minor rather than either `1.0.0` or the blanket exemption semver grants `0.y.z`. At `1.0.0`
  the shift ends and the implied impact is the requirement. It carries a schema id
  (`majordomus/version-policy/v1`) so that a verdict recorded today stays explicable when the
  policy changes.
- **Conventional commits become evidence.** They stay the changelog's prose and the human
  account of a change; they are carried in the plan beside the verdict, and when they
  classify a window as less than the contract moved by, the plan says `understated` rather
  than deferring to them. This is the line the subsystem exists for: a human label and a
  measured fact, disagreeing, in the open.
- **One plan, applied rather than recomputed.** `release bump` does not compute anything. It
  reads the same `VersionPlan` the command line prints, `GET /api/v1/release/analysis`
  answers, the `majordomus_release_analysis` MCP tool returns and the CI gate exits on, and
  its whole job is `plan → validate → apply(plan)`. `--level` and `--exact` may name a
  version above the measured floor, reported with its provenance, and are refused below it
  with nothing written.
- **The gate holds no semantics.** `scripts/ci/version-matches-surface` is an adapter: it
  runs `majordomus release analyze` and maps the exit code. It moves to the `rust` job, which
  has the executable and the full history, and the `rust`, `rust-generated` and
  `distribution` path classes select it, so a change that can move the contract is asked
  before it lands.

## Alternatives rejected

**Keep both and make them agree.** The obvious minimum: leave the shell comparison as the
gate, leave commit inference in the writer, and add a check that the two answers match. It
would encode the belief that two implementations of one rule can be kept in step by a third
thing watching them, which is the belief `derived-once` exists to refuse. It also could not
work in the direction that matters: the writer runs on a developer's machine before the gate
exists, so the disagreement would still be discovered after the number was chosen.

**Make the gate schema-aware in shell.** Cheaper than a Rust engine, and it fails on the
first case that matters. Schema compatibility is directional and recursive; implementing it
over `jq` would produce a program nothing could unit-test, in a language with no types, for a
question where a wrong answer is silent. The compatibility matrix in
`release/compat.rs`’s test module has thirty-odd rows, and every one of them is a test that could not
have been written against a shell pipeline.

**Go Elm-strict and start at 1.0.0.** Elm's actual rule: every package begins at `1.0.0`, so
breaking changes always cost a major and there is no `0.x` hatch at all. It is the more
honest policy and this repository has not earned it — `1.0.0` is a promise about the shape of
the contract, and the contract is still moving weekly. Taking the number without the promise
would make `1.x` mean less here than `0.x` means with a measured floor. The policy carries a
mode and a schema id precisely so this can be revisited as a change to one declaration.

**Commit the analysis as a generated artifact.** Every other projection in this repository is
written to `docs/generated/` and gated by `generate --check`, so a `release-analysis.json`
looks like the consistent choice. It is the one shape that cannot work here: the analysis is
a function of HEAD, so a file describing it is stale the instant it is committed — it would
have to be regenerated by the commit that invalidates it, in every commit, forever. The
changelog already pays a smaller version of this tax and it is the slowest part of the
pre-commit path. The analysis is served live instead, by the four surfaces that can be asked
at the moment the question matters, and the committed `registry.json` it reads is the durable
artifact. What *is* durable about a given release — the surface fingerprint and what moved —
belongs in that release's record, written once, by the pipeline, when there is something to
record.

**Infer renames and collapse them.** A rename appears as a removal and an addition, which
reads as two changes for what a person thinks of as one. Detecting it and reporting a single
"renamed" entry would be friendlier and would lose the half that matters: a caller holding
the old name is broken, and that is a major whatever the tidier report calls it. The two
entries stay, and the removal's detail names what it moved to.

## Consequences

The version is now a measurement, and the number cannot be smaller than the truth without a
gate refusing it before the merge. `release analyze` is a public capability, so the verdict
reaches the command line, HTTP, OpenAPI, MCP and the Cockpit as a projection of one value
with no second registration — and adding it obliged this change to raise its own version,
which is the subsystem's first real use.

Four costs are real. The gate needs the executable, so it costs a build in a job that has one
rather than running in the toolchain-free structure job; it moved rather than gained a
dependency, since the alternative was a second implementation. Conservatism on unmodelled
schema keys will sometimes demand a bump nobody thinks is breaking — the diagnostic names the
pointer and the key so the answer is arguable, and below `1.0.0` the cost of being wrong that
way is a minor. The baseline is now the newest release *record* rather than the newest tag,
which is the canonical source but means a release published without a record is invisible to
it; the plan reports `tag-without-record` and `record-without-tag` in both directions rather
than picking whichever looked newer, and three such tags existed when this was written. And
a release before the registry was committed cannot be compared with at all, which is stated
as a refusal rather than absorbed as an empty diff.

What holds it: the compatibility matrix and the policy tables in
`apps/majordomus-cli/src/release/compat.rs`, including both policy modes and the
determinism invariants; the surface reader's own tests over the fingerprint, the
normalisation and every way a baseline can be unreadable; `test/cases/251_version_is_measured.sh`
over the whole subsystem against the real executable in a repository built for the purpose —
an additive change, a breaking change, an internal refactor, an override that undershoots and
one that overshoots; and `scripts/ci/version-matches-surface`, which is now the gate and the
engine at once rather than a second opinion.

This **amends** ADR 0029 rather than replacing it, and the distinction is not bookkeeping.
Supersession in this layer is total and bidirectional — a superseded record is withdrawn
whole, and must name the decision that withdrew it. Three of ADR 0029's four decisions are
still in force and still correct: the changelog is composed rather than authored, the window
is one interval said twice, the version keeps two statements and one writer. Only its fourth
— that the writer's default is the bump the conventional commits imply — is replaced here,
and withdrawing the whole record to say so would make three standing decisions unfindable.
ADR 0029 carries the amendment note and stays `accepted`.
