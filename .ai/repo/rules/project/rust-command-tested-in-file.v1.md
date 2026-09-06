---
id: project.rust-command-tested-in-file
version: 1
kind: rule
title: A command is granular, tested and documented in the file that declares it, and composed rather than registered
description: Every capability module the executable composes carries its own tests beside the declaration and at least one executed doc example, is reached by the root composition rather than by a registration elsewhere, and is held to a declared coverage floor.
statement: Declare a command in one module, test it and document it in that same file with tests that run and examples that execute, and let the composition reach it; a command whose evidence lives somewhere else, or that something must be told about, is not finished.
status: active
class: advisory
depends_on: [project.no-claim-without-test@1, project.interfaces-are-projections@1]
tags: [rust, command, testing]
x-majordomus:
  validator: rust_command_tested
  category: rust-command
  enforced_by: [doctor]
  exit_code: 10
  tests: [test/cases/88_rust_command_tested.sh]
---

# Rationale

A command is the smallest thing this executable offers a person, and it is where evidence is
cheapest to write and most often written somewhere else. The crate already keeps its tests
beside the code that earns them — the web module holds its own path safety, its own
ordering, its own refusals — and it is measurably the part of the crate that is easiest to
change safely. The capability modules, which are the commands themselves, are the exception:
they declare the surface a user touches and carry no tests at all, so their evidence lives in
suites that name them from a distance.

Distance is the problem, not quantity. A test in a suite covers the command as it was when
somebody last thought about that suite; a test in the file is read by whoever changes the
declaration, in the same screen, and is the first thing to go red. A doc example is stronger
still, because `cargo test --doc` runs it: documentation that is executed cannot drift into
describing a command that no longer behaves that way, which is the failure mode prose has and
tests do not.

Granularity is what makes both affordable. One capability per declaration, one module per
subject, means a test names one behaviour and a reader looking for it opens one file. A
module that grows into several unrelated commands makes every test in it ambiguous about
what it protects.

The last part is composition. A command is reached because the root composes its module, not
because a list somewhere was told it exists — the same property `project.interfaces-are-projections`
requires of every other surface. A module nobody composes is a command that exists and is
served to nobody, and a module reached through a hand-kept registration is a line somebody
will forget.

# Required behaviour

Every module the root composition composes lives in its own file, declares its capabilities
there, and carries in that same file at least one `#[test]` and at least one doc example.
The tests exercise the module's own behaviour rather than restating the registry's; the doc
example is executable, because an example that is not run is prose.

A module that declares itself is composed by the root, and composition is the only way a
command is reached. Adding a command to an existing module adds no line anywhere else;
adding a module adds exactly one, in the composition.

The crate is held to a declared coverage floor, which lives in a file rather than in a
habit, and the coverage gate reads it. Coverage is a floor and not a target: it says which
changes may not land, not how much testing is enough.

# Failure behaviour

`majordomus doctor` dispatches `mj_validate_rust_command_tested`, which reads the composed
module list from `compose_modules!` itself and reports, per module, a missing file, an
absent in-file `#[test]` and an absent doc example; across the tree, a module declared with
`module!` that the root does not compose; and, once, an undeclared or unreadable coverage
floor. It is `advisory` at version 1: the modules it measures do not satisfy it yet, and a
blocking rule would stop every commit in a repository for a debt it did not create. The
findings are reported on every `doctor` run so the debt is visible rather than agreed to in
silence. Promotion to `blocking` belongs in version 2, once the composed modules carry their
own evidence.

Where there is no Rust crate the doctrine is skipped and says so: the layer installs into
repositories that carry no executable, and a doctrine that cannot apply is not a violation.

# Verification

`bash test/run.sh 88_rust_command_tested`, which builds a fixture with a composed module
that has tests and examples, one that has neither, and one declared but composed by nobody,
and asserts that the validator finds exactly the second and third; it also asserts the skip
in a repository with no crate, and that the module list is read from the composition rather
than from the validator. `majordomus doctor` reports the state of this repository.
