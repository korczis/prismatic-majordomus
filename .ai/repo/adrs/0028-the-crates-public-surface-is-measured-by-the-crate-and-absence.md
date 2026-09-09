---
schema: adr/v1
id: adr-0028
kind: adr
title: The crate's public surface is measured by the crate, and absence from the API is a decision
status: accepted
date: 2026-09-08
tags:
  - rust
  - documentation
  - testing
  - capabilities
  - quality
related:
  - rule:project.rust-public-api-quality
  - rule:project.operation-transport-parity
  - rule:project.interfaces-are-projections
  - rule:project.rust-command-tested-in-file
  - rule:project.no-claim-without-test
  - file:apps/majordomus-cli/src/quality/mod.rs
  - file:apps/majordomus-cli/src/quality/source.rs
  - file:apps/majordomus-cli/src/quality/policy.rs
  - file:apps/majordomus-cli/src/quality/parity.rs
  - file:apps/majordomus-cli/src/cli/local.rs
  - file:apps/majordomus-cli/src/capability/builtin/quality.rs
  - file:docs/QUALITY.md
provenance:
  origin: authored
---

# 28. The crate's public surface is measured by the crate, and absence from the API is a decision

## Context

Everything this executable offers is a projection of one capability registry. ADR 0002
established that, `project.interfaces-are-projections` requires it, and `tests/projections.rs`
proves it in both directions for MCP, HTTP and OpenAPI. Two things sat outside that
arrangement, and both were invisible for the same reason: nothing measured them.

**The crate's own Rust surface.** `#![warn(missing_docs)]` with `-D warnings` in the gate
makes an undocumented exported item a build failure, and on the day this decision was made
every one of the crate's exported items was documented. That is the whole of what the
compiler can say. It cannot say whether the documentation is worth reading, and it says
nothing at all about examples — where the interesting failure lives, because prose about
behaviour drifts silently and an executed example cannot. Measured for the first time, the
crate had 3595 exported items, 1191 of which carried behaviour, and 149 of those had an
executable example. Of 131 exported modules, 9 carried an example and 36 had no test beside
them and no test naming them. `majordomus quality report --summary` computes the current
numbers; the ones here are what was true on the day and are not maintained.

**The command line.** MCP, HTTP and OpenAPI declare nothing of their own, so on those three
there is nothing to go missing. The command line is different: it carries commands beyond
the capabilities — it starts servers, it writes generated files, it renders values for a
person — and it had 53 runnable commands of which 17 were bound to a capability and 36 were
bound to nothing. Reading them one by one, most were fine and a few were not, and there was
no way to tell those groups apart except by reading them one by one.

The third fact is the one that shaped the answer: this crate is `publish = false`. Its
consumers are its own binary, its tests, its benches and its doc examples. There is no
external consumer at all, so most of that 3595 was not an API anybody had decided to offer.
It was `pub` because `pub` is what one types.

## Decision

**The measurement is a capability of the crate, and the crate is its subject.**
`src/quality/` reads the crate as a syntax tree — [`syn`], walked from `src/lib.rs` the way
`rustc` walks it, so effective visibility is computed and not guessed — and produces one
typed `QualityReport`. `quality.report` is a capability like any other, so the command line,
HTTP, the OpenAPI document, MCP and the Cockpit are projections of one execution. A quality
subsystem that needed five renderers would be failing the architecture it exists to enforce.

**What an item owes is derived from the item, and what it does not owe is printed.** A
module, a type, a trait, a function, a method and an exported macro carry behaviour and owe
an executable example. A field, a variant, a constant, a static, a type alias and an
accessor that hands back what the receiver holds do not: an example of a value is an example
of whatever reads it, and demanding one buys `assert!(true)` inside a fence. That
classification is computed from the item's kind and shape and reported as an exemption with
its reason. **There is no exemption file**, because an exemption file is a place to add a
line.

**An example counts when a machine can tell that it is one**: the toolchain compiles it
(`no_run` counts, `ignore` does not), it names the item it documents, and it asserts
something that could be false — `unwrap`, `expect` and `?` are assertions because a failure
of either fails the doctest, and `assert_eq!(1, 1)` is not one. That is deliberately not a
judgement about prose. It is the smallest set of tests that makes the obvious ways of
satisfying a counter fail, and `tests/quality.rs` has one fixture crate per way, so the
validator cannot quietly become ceremonial.

**A command is the projection of a capability, or it says why it is not.** `cli::LOCAL`
lives beside the clap declaration and gives one of five reasons: the command becomes a
process, it writes into the repository, it answers about the caller's own checkout, it is
another name for a command already accounted for, or it renders a capability — in which
case it names the capability, and the check refuses the claim unless that capability exists
and answers over HTTP. There is deliberately no reason meaning *not projected yet*: a read
that belongs in the API and is missing from it is a defect, and a vocabulary that can
express it politely is a vocabulary that will.

**A strict rule about the surface is a rule about its size.** An item that no consumer needs
costs a doc comment and an example, and `pub(crate)` is the cheaper answer. That pressure is
the point and not a side effect: it is why the finding for a missing example offers reducing
visibility as a remedy in the same sentence as writing one.

## Alternatives

**A shell script over the sources.** The repository already had one for a narrower question
(`scripts/ci/rust-command-check`, `project.rust-command-tested-in-file`), and it works
because its subject is a macro invocation on its own line. This question is not line-oriented:
`pub fn` in a private module exports nothing, a `pub use` carries a type out of one and
leaves its private members behind, and a fenced block is a test only under conditions that
are syntactic. A parser was the only honest answer, which is why `syn` is a dependency and
`grep` is not.

**rustdoc JSON.** It answers exactly this question and is nightly-only. The repository builds
on stable and pins no nightly, and a gate that needs a second toolchain is a gate that gets
skipped.

**Demanding an example of every exported item, with no classification.** It is the rule as
one would first state it, and it produces 1500 examples of constants and struct fields, all
of them `assert!(true)`. The classification is what makes the rule enforceable at all.

**A `warn`-forever mode.** Rejected: a warning nobody has to act on is a measurement, not a
rule, and this repository has enough measurements.

## Consequences

The exported surface fell from 3595 items in 131 modules to 3106 in 79, by demoting the
modules no consumer names to `pub(crate)` — fifty-five of them, of which three came back
because their own doc examples name them, which is the criterion working rather than an
exception to it. That reduction is not cosmetic: it is what makes the remaining rule
affordable, and it removed nine items of dead code that being `pub` had hidden from the
compiler's own `dead_code` analysis.

Every exported module is now behaviourally tested, which it was not when this was written.
The rest of the debt is the example policy, and it is recorded rather than argued with.

Two of those nine turned out to be used by in-crate tests and were restored as
`#[cfg(test)] pub(crate)`, which is where test support belongs. The lesson is recorded here
because it will recur: `cargo build` does not compile `#[cfg(test)]` code, so a visibility
reduction has to be checked with `cargo check --all-targets`, and the dead-code warnings
from a plain build are not the whole truth.

The remaining debt is real and is recorded, not hidden: `.ai/repo/rust-quality-baseline.txt`
holds the findings that stood when the rule landed, keyed by code, file and symbol but
deliberately not by line, so a finding that moves down its file is the same finding. The
gate fails for a finding outside it, so the debt can shrink and cannot grow, and a stale
entry fails too, so debt that is paid must leave the file. That is the mechanism
`project.rust-command-tested-in-file` introduced and then retired when its debt reached
zero; this rule is expected to follow it.

## Migration

The rule landed against an existing tree, so it landed with the ratchet. New code is held to
it in full from the first commit — the gate caught this change's own new Cockpit page before
it was committed, which is the demonstration that matters. Existing debt is worked down file
by file, and the rule is finished when the baseline is empty and the file is removed; an
empty exemption list invites somebody to add a line to it.

## Enforcement

`majordomus quality report` measures and exits `10` on any finding outside the baseline. The
`rust-quality` gate in `.ai/repo/ci/gates.yaml` runs it, `scripts/rust-check` runs it in the
same order a person does, and `apps/majordomus-cli/tests/quality.rs` asserts the ratchet in
both directions from inside the crate's own suite. `tests/quality.rs` also carries the
negative fixtures: one crate per way of failing, each producing its own code and no other.

## Extension model

A new capability needs a typed input and output, a handler, a `capability!` declaration and
a test beside it; the projections follow, as they did before this decision. What is new is
that a new *command* must be one of those or must say why not, and a new exported Rust item
must explain itself and show itself running — or stop being exported, which is usually the
right answer and is offered as the remedy in the finding itself.
