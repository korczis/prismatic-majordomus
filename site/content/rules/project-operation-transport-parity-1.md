+++
title = "A command is the projection of a capability, or it says why it is not, and the reason is checked"
description = "A command is the projection of a capability, or it says why it is not, and the reason is checked"
weight = 103
[extra]
kind = "rule"
slug = "project-operation-transport-parity-1"
identity = "project.operation-transport-parity@1"
status = "active"
source = ".ai/repo/rules/project/operation-transport-parity.v1.md"
+++
{% raw %}

## Rationale

`project.interfaces-are-projections` already says a capability is defined once and every
interface derives from it, and the crate's suites already prove that in both directions for
MCP, HTTP and OpenAPI: a declared projection exists, and no projection carries an entry the
registry does not hold. On those three transports there is nothing left to go wrong,
because none of them has anything of its own to declare.

The command line does. It is the one projection that carries commands beyond the
capabilities — it starts servers, it writes generated files, it renders values for a person
— and so it is the one place where an operation can end up outside the API without anybody
choosing that. Measured on the day this rule landed, the command line had fifty-three
runnable commands; seventeen were bound to a capability, and thirty-six were bound to
nothing at all. Reading them one by one, most were fine and a few were not, and there was
no way to tell those two groups apart except by reading them one by one, which is the
condition this rule ends.

The fix is not to force every command into an HTTP route. `majordomus serve` becoming an
endpoint would be asking a server to become a different server; `majordomus generate`
writing files over a read-only API would break the executable's first promise. The fix is
that **absence is a statement somebody made and a machine checks**, rather than a gap.

The vocabulary matters more than the mechanism. There are four structural reasons a command
belongs to the command line alone in this executable — it becomes a process, it writes into
the repository, it answers about the caller's own checkout, or it is another name for a
command already accounted for — and a fifth for *renders a capability*, which is not an
absence at all: the operation is in the API and the command is its terminal rendering, so
the classification names the capability and the check refuses the claim unless that
capability exists and answers over HTTP. There is deliberately no reason meaning "not
projected yet". A read that belongs in the API and is missing from it is a defect, and a
vocabulary that can express it politely is a vocabulary that will.

## Required behaviour

Every runnable command of `cli::tree()` — a leaf, or a parent that does something when no
subcommand follows it, both read from clap's own declaration and never from a list — is
either bound to a capability through that capability's `CliExposure`, or carries an entry in
`cli::LOCAL` giving one of the five typed reasons and one sentence about that command in
particular.

A command that only groups others is not a command for this purpose: it runs nothing, so
there is no operation for it to be or to be missing, and a command that grows a required
subcommand stops being runnable on the same edit that makes it so.

`cli::LOCAL` lives beside the clap declaration it is about, for the reason `cli::EXAMPLES`
does: the person adding a command is looking at that file, and a statement kept anywhere
else goes stale in a refactor nobody connects to it.

Every capability that declares an HTTP route is described by the generated OpenAPI
document, and every operation the document describes is declared by a capability.

## Failure behaviour

`majordomus quality report` reports, with the codes `OPERATION_CLI_UNCLASSIFIED` for a
command that is neither, `OPERATION_CLASSIFICATION_STALE` for an entry naming a command,
a capability or an alias target that is not there, `OPERATION_CLASSIFICATION_CONFLICT` for a
command that is both bound and classified, `OPERATION_MISSING_OPENAPI` for a declared route
the document does not describe, and `OPERATION_PROJECTION_ORPHAN` for an operation the
document describes and no capability declares. The command exits `10` when any stands, and
the `rust-quality` gate in `.ai/repo/ci/gates.yaml` runs it.

The three ways of getting this wrong are the three findings, and none of them can be reached
by forgetting: adding a command raises the first, removing one raises the second, and
promoting a classified command to a capability without removing its entry raises the third.

## Verification

`cargo test --manifest-path apps/majordomus-cli/Cargo.toml quality::parity`, which runs the
inspection against the real registry and the real command tree and asserts that no entry of
`cli::LOCAL` is stale, that a pure group is not treated as a command, and that an OpenAPI
document missing a declared route is reported once per route.

`apps/majordomus-cli/tests/quality.rs`, which asserts the property this rule exists for:
every runnable command is accounted for exactly once, and the two accountings do not
overlap.
{% endraw %}
