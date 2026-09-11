---
id: project.canonicality
version: 1
kind: rule
title: Every concept has exactly one canonical source of truth
description: Every concept in Majordomus has exactly one canonical source of truth; everything else is derived, discovered, generated, projected or mechanically synchronised from it; a second representation kept by hand is a defect, and an architecture in which two representations must be kept in sync by hand is incomplete.
statement: A concept has one canonical source; every other representation of it is derived from that source by a mechanism, is identifiable as derived, and is checked against the source; a hand-kept second representation is a defect; and a new feature names its canonical source before its implementation is accepted.
status: active
class: blocking
depends_on: []
tags: [canonicality, derived, architecture, meta]
---

# Rationale

Every other rule this repository holds about sources and projections is a case of one
principle, and until that principle was written down each case argued for itself: the
capability registry against a hand-written MCP table, the distribution model against the
installer's platform list, the why catalogue against the pages that rendered it, the
generation manifest against the tree it indexed. Each was won separately, each with the
same argument, and the argument deserves to be stated once, above them, so that the next
case is decided by reading it rather than by fighting it again.

The argument is this. A thing written down twice is a thing that is wrong in one place
the moment it changes in the other, and nothing tells anyone which place. The repair is
never "be more careful": it is to make one of the two the source and turn the other into
something a mechanism produces from it. What is derived cannot drift, because it is not
kept; it is regenerated, and a check names it stale when it was not.

This is the root doctrine. The rules that depend on it — interfaces are projections,
derived once, the Rust declaration is canonical, generated artifacts are typed, the
distribution is one model, the why catalogue is canonical, a web surface is declared
once — are its instances for one area each. A new area gets a new instance, or is
covered by this one directly; it does not get an exception by omission.

# Required behaviour

**One source.** Every concept — a capability, a command, a platform, a page, a rule, a
document, a schema, a piece of knowledge — has exactly one place it is declared. That
place is its canonical source. The knowledge model (`docs/KNOWLEDGE.md`) can name it for
every capability: `majordomus explain capability <id>` prints the canonical source first.

**Everything else derived.** Every other representation is one of, in order of
preference:

1. **derived at run time** from the source, by the executable that serves it — the MCP
   tools, the HTTP routes, the OpenAPI document, the Cockpit pages;
2. **generated at build time** and never committed — the site's pages over the registry
   dataset;
3. **generated and committed**, with a provenance header, a typed declaration in the
   manifest naming the source it was derived from (`derived_from`), and a check that
   refuses it stale — everything under `docs/generated/`, the allow-lists, the provider
   bootstraps;
4. **mechanically synchronised** — never. Two representations that a person must keep in
   step by hand are the defect this rule exists to remove.

**Identifiable.** A derived representation says so: the header its encoding allows, the
manifest entry, the `merge=derived` attribute for a tree a merge must not resolve by
hand. A reader can always tell what is authored from what is produced.

**Checked.** The derivation is validated mechanically, not trusted: `majordomus generate
--check` for the committed artifacts, `majordomus canonicality check` for the whole
architecture — every capability's source and derived surfaces, every generated file
without a declared source (an orphan projection), every file in a generated tree that no
plan declares, every hand-written listing of capability identifiers (a suspected
mirror), every expired exception.

**Named before built.** A change that adds a capability, a kind, a surface or a document
names its canonical source before the change is accepted: `majordomus change inspect`
lists every capability the change adds with the surfaces derived for it, and the
pull-request review reads that list. A feature whose source is "the table in the
documentation" is refused.

**Measured.** The manual maintenance surface (MMS) of a capability is the number of
hand-written files a person must touch to change it: its declaration counts one, every
hand-written file that names it one more, generated files none. The goal is one. The
audit prints the mean and the count at one, and the number is allowed to move in one
direction.

**Excepted explicitly, for a while.** A violation that stands for a reason is written
down as a typed exception under the knowledge section — id, reason, owner, expiry date,
and how to tell it is no longer needed — and it counts again the day it expires. What the
repository had before this rule is recorded in the knowledge baseline as tolerated debt,
and that debt may only shrink.

# Failure behaviour

`majordomus canonicality check` exits 10 naming every violation that neither the baseline
tolerates nor an exception covers. `majordomus knowledge check` carries the same
violations as canonicality gaps and refuses new ones in `protect` mode. Both run in the
`rust-integration` gate of CI and in the pre-commit hook, so a hand-kept mirror is refused
before it is merged, not found afterwards.

# Verification

- `apps/majordomus-cli/tests/knowledge.rs` — the audit finds an orphan projection, a
  suspected mirror and an expired exception in a fixture, and passes when each is
  tolerated, excepted or removed.
- `test/cases/99_knowledge.sh` — the same through the built executable, in a disposable
  repository, with the exit codes.
- `majordomus canonicality check` in this repository, in CI, on every push.
