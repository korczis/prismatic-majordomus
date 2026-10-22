---
schema: adr/v1
id: adr-0121
kind: adr
title: A test criterion names a test where the repository keeps its tests, and the repository says where that is
status: proposed
date: 2026-10-09
tags:
  - evidence
  - intent
  - adoption
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:.ai/repo/adrs/0107-completion-discharges-obligations-and-never-decides-whether-an-intent-is-satisfied.md
  - file:apps/majordomus-cli/src/evidence/mod.rs
  - file:apps/majordomus-cli/src/intent.rs
  - test:test/cases/983_an_adopter_runs_the_intent_loop_on_its_own_tests.sh
---

# 121. A test criterion names a test where the repository keeps its tests, and the repository says where that is

## Context

An intent's criterion of evidence kind `test` names a test by path, and the evidence engine turns
that path into a test identity: `TestId::of` maps `test/cases/<name>.sh` to the suite runner and
`apps/majordomus-cli/tests/<name>.rs` to the crate runner, and `TestId::source` maps back. Those
two directories are where the tool's own repository keeps its tests. Every other path maps to
nothing.

In a repository that adopted the tool and keeps its tests in `tests/`, `spec/`, `__tests__/` or
beside the code, a `test` criterion therefore names no test: `intent validate` fails it with
"give a path under test/cases/ or apps/majordomus-cli/tests/", and a recorded run of the
adopter's own test is reported as "no such test here". The verdict of such an intent can never be
`satisfied`. Reproduced on 2026-10-09 in a fresh `init` with `tests/greet_test.sh`. The rule the
whole intent subsystem keeps — the answer is derived from the repository's state, never from the
tool's own identity — is broken at its first step.

## Decision

**The repository declares where its tests are and how one is run again; the declaration
defaults to today's two.** The policy gains `evidence.tests`, a list of `{runner, root, suffix,
run}`: a test under `root` whose file name ends in `suffix` is the test `<runner>:<name>`, where
`<name>` is the file name without the suffix, and `run` is the argv that runs that one test
again, with `{name}` or `{path}` in place of the test — an argv, never a shell string, by the rule
ADR 0119 applies to commands. When the key is absent the list is the built-in
`[{suite, test/cases, .sh, [bash, test/run.sh, {name}]}, {crate, apps/majordomus-cli/tests, .rs,
[cargo, test, --test, {name}]}]`, so this repository, and every repository that never writes the
key, behaves exactly as before. `TestId::of`, `TestId::source`, `TestId::reproduce` and the
recorder read the declaration through one resolver; nothing else maps a path to a test or says how
to run it. `suite` and `crate` are the two built-in runner kinds; what a declared root means — where
its tests are and how one is run — comes from the declaration, not from the runner's name, so a
repository whose tests are pytest or jest declares them under `suite` with its own `run`, and every
surface prints that command as the reproduce line instead of a false `bash test/run.sh`.

**Three constraints, so that the declaration extends the runners and never forks them** (asked
for by ADR 0119):

1. The runner is one enum. A declaration names a runner the enum knows and gives it a root; it
   never introduces a runner. `command` and `deployment` join that enum in ADR 0119 without a
   second registry.
2. A reference with a prefix — `suite:<name>`, `crate:<binary>`, and ADR 0119's `command:` and
   `deployment:` — is resolved by its prefix before any root is consulted; a declaration never
   swallows a prefixed reference as a path.
3. `evidence record` refuses a row whose runner is neither built in nor declared, and a path-based
   row whose name maps to no declared root, naming the declaration to add — so a hand-written row
   is not evidence because it looks like one. A prefixed runner of constraint 2 (`command:`,
   `deployment:`) has no root: it is resolved by its prefix and refused unless its own recorder
   wrote it, as ADR 0119 decides.

**One root per runner, flat.** A declaration that names a runner twice is refused by `doctor` and
`intent validate`, and a name with a separator is not a test name, as today. A repository whose
tests nest is a later decision; the limit is stated, not hidden.

**The refusal teaches the declaration.** `intent validate` on a criterion whose path no root
covers names the path and the policy key to add, instead of the tool's own directories.

## Consequences

- An adopter's intent can be satisfied by its own tests, recorded with the documented
  `evidence stamp` and `evidence record`.
- This repository changes nothing: no key, the built-in default, the same ids.
- The skeleton's policy documents the key, commented, with this repository's default spelled out.
- One root per runner: a repository with tests of one kind in two directories, or in two languages
  under one runner, cannot express both yet, and a nested layout cannot be expressed at all. Both
  are stated limits, measured when an adopter needs them, not hidden.

## Alternatives rejected

- **Inferring the layout from files on disk:** two conventions in one repository, or none, would
  give a different answer on every machine; a declaration is read the same everywhere.
- **Accepting any path as a test:** a criterion would be met by recording a run of any file, and
  the engine could not say which runner produced it.
- **A second registry of runners for adopters:** two vocabularies for one concept; ADR 0119 needs
  the same enum.
