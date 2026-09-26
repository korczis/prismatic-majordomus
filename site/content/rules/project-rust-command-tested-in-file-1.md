+++
title = "A command asserts in the file that declares it that it is what it claims, and is composed rather than registered"
description = "A command asserts in the file that declares it that it is what it claims, and is composed rather than registered"
weight = 122
[extra]
kind = "rule"
slug = "project-rust-command-tested-in-file-1"
identity = "project.rust-command-tested-in-file@1"
status = "active"
source = ".ai/repo/rules/project/rust-command-tested-in-file.v1.md"
+++
{% raw %}

## Rationale

A command is the smallest thing this executable offers a person, and it is where evidence is
cheapest to write and most often written somewhere else. The crate already keeps its tests
beside the code that earns them — the web module holds its own path safety, its own
ordering, its own refusals — and it is measurably the part of the crate that is easiest to
change safely. The capability modules, which are the commands themselves, are the exception:
they declare the surface a user touches, most of them assert nothing about it, and their
evidence lives in suites that name them from a distance.

Distance is the problem, not quantity. A test in a suite covers the command as it was when
somebody last thought about that suite; a test in the file is read by whoever changes the
declaration, in the same screen, and is the first thing to go red. A doc example is stronger
still, because `cargo test --doc` runs it: documentation that is executed cannot drift into
describing a command that no longer behaves that way, which is the failure mode prose has and
tests do not.

Granularity is what makes both affordable. One capability per declaration, one module per
subject, means a test names one behaviour and a reader looking for it opens one file. Most
of that is already enforced and this rule does not restate it: the registry refuses a
capability declared outside its own module's namespace, and refuses a duplicate identity —
`capability-modules`, proved by case 91. Repeating a check that already exists would be the
second registry this repository spends its time removing. What the registry cannot see is a
module composed into the application that declares no command at all, and that is the one
granularity finding this rule adds.

The coverage floor is part of the same argument, and there is a case for it from this
repository rather than from principle. A branch that added a model, a static file server and
a command surface came back at 89.40% against a floor of 90. The floor did not find
anything — it refused to move, and that refusal is what sent its author looking. What turned
up was in code written an hour earlier and believed to be covered: vocabulary whose `Display`
and serialisation could have disagreed, an empty selector that could have meant "none"
instead of "every", a root-mounted static surface, a surface with no index. A target rather
than a floor would have allowed 89.40 to be rounded up to basically ninety, and four
untested branches would have shipped in a file whose whole job is deciding which surface
answers a request.

The last part is composition. A command is reached because the root composes its module, not
because a list somewhere was told it exists — the same property `project.interfaces-are-projections`
requires of every other surface. A module nobody composes is a command that exists and is
served to nobody, and a module reached through a hand-kept registration is a line somebody
will forget.

## Required behaviour

Every module the root composition composes lives in its own file, declares its capabilities
there, and carries in that same file at least one assertion that runs: an in-file `#[test]`,
or a doc example, which `cargo test --doc` executes. Either form satisfies the rule and the
choice belongs to whoever writes it — demanding a particular form buys a token test beside a
real example, which is ceremony rather than evidence.

What the assertion must be about is the declaration: that the command is what it claims. A
module that only wires an already-tested subsystem is not exempt, because the wiring is
exactly what nothing else checks — the model's own suites test the model, not its
projection, and a capability that quietly loses an exposure in a refactor breaks no test
that lives beside the model. If a declaration is so thin that no assertion about it would
mean anything, that is a reason to doubt the module, not the rule.

A module that declares itself is composed by the root, and composition is the only way a
command is reached. Adding a command to an existing module adds no line anywhere else;
adding a module adds exactly one, in the composition.

The crate is held to a declared coverage floor, which lives in a file rather than in a
habit, and the coverage gate reads it. The floor is a number and it is a ratchet: a committed
lower bound that may rise and never fall. This rule reads `scripts/rust-coverage-threshold`
and refuses a floor below the bound written once, as `FLOOR_BOUND`, in
`scripts/ci/rust-command-check`; the bound is the crate coverage measured when it was set,
and it is raised together with the file whenever coverage improves. Lowering either is a
visible edit to a blocking gate rather than a quiet change that leaves every other check
still passing. Coverage is a floor and not a target: it says which changes may not land, not
how much testing is enough.

The floor was once a fixed ninety, and that failed as a gate. Every recorded coverage job
measured the crate between 83.10 and 83.34 percent of lines with test code out of the
denominator, and no run ever met ninety, so the coverage job was red on every push and its
verdict said nothing about any change. A floor no run meets gates nothing; the measured value,
held so it cannot fall, refuses every regression. What a ratchet on the aggregate does not do —
refuse new uncovered code while old debt keeps the number flat — is held by the changed-code
gate, `project.new-code-is-covered`, which covers every line, function and region a change
adds or touches whatever the aggregate says.

## Failure behaviour

`scripts/ci/rust-command-check` reads the composed module list from `compose_modules!`
itself and reports, per module, a missing file, a module that declares no capability, and
the absence of any assertion that runs; across the tree, a module declared with `module!`
that the root composes nowhere; and, once, a coverage floor that is undeclared, not one
integer, or below its ratchet bound `FLOOR_BOUND`. The `rust-command` gate in `.ai/repo/ci/gates.yaml` runs it.

The gate runs `--strict`: every composed module must satisfy the rule, and one that does not
fails the build. There is no baseline and no exemption list, because there is nothing left to
exempt — every module the application composes now asserts, in the file that declares it,
that it yields the identity and the projections it claims.

It did not start there. The rule landed with a ratchet: the modules that did not satisfy it
were recorded in a baseline written only by `--write-baseline`, and the gate failed only for
a module absent from that list, so the debt could shrink and could not grow. That mechanism
is still in the check and is the right way to introduce a rule against an existing tree — a
blocking rule with no migration path is a rule that gets reverted. It is simply no longer
needed here, and the baseline file was removed rather than left empty, because an empty
exemption list invites somebody to add a line to it.

The validator does not live in `lib/`. That library belongs to the shipped tool, and
`scripts/generate-site-data` refuses a `mj_validate_*` function that no shipped doctrine
declares — rightly, because the site documents the product's doctrines and this rule is this
repository's own. A project rule that needs a check gets a gate, not a doctrine.

Where there is no Rust crate the doctrine is skipped and says so: the layer installs into
repositories that carry no executable, and a doctrine that cannot apply is not a violation.

## Verification

`bash test/run.sh 88_rust_command_tested`, which builds a fixture holding a composed module
with a test, one with only a doc example, one with neither, and one declared but composed by
nobody, and asserts that the check reports exactly the last two — so that either form of
assertion is proved to satisfy the rule rather than only claimed to. It also asserts that a
composed module declaring no command is reported, that a floor which is not one integer or
sits one under the bound the check declares is reported and fails the gate while a floor at or
above the bound is not, that a module already on the baseline does not fail the gate while one that is not
does, and that a repository with no crate is passed over rather than failed.

`scripts/ci/rust-command-check` on this repository reports the modules still owing an
assertion and exits zero while they are the ones the baseline records.
{% endraw %}
