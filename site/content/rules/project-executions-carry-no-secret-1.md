+++
title = "A sensitive value never enters an execution"
description = "A sensitive value never enters an execution"
weight = 86
[extra]
kind = "rule"
slug = "project-executions-carry-no-secret-1"
identity = "project.executions-carry-no-secret@1"
status = "active"
source = ".ai/repo/rules/project/executions-carry-no-secret.v1.md"
+++
{% raw %}

## Rationale

An execution's input is written down. It goes into the snapshot, into the first event, into
the retained history, onto a page in the Cockpit, into a listing, and out over a live channel
to whoever is watching — six places, from one assignment. A redaction performed in any one of
them protects that one; the value is already in the other five.

The other half of the rule is what decides *what* is sensitive. Matching a field's name is
the usual answer and it is wrong in both directions: it destroys a `password_policy` that is
a document name, and it misses a `credential` a future field spells differently. The type
already declares everything else a projection reads — the description, the constraints, the
examples — and it can declare this, in the schema, where the redactor and the reader of the
API reference see the same statement.

## Required behaviour

Sensitivity is declared on the input type's schema: `format: "password"`, `writeOnly: true`,
or the explicit `x-majordomus-sensitive: true`. Redaction walks the schema, follows internal
references, reaches nested objects, arrays and maps, and replaces the value with a marker
that reads as withheld rather than as absent.

It happens once, in the execution engine, before the input is stored, so that everything
downstream is reading an already-safe value. No transport, page, log line, generated document
or client may redact again, and none may be relied on to. A field's name must never decide
it, and there must be no second list of patterns anywhere in the crate, the scripts or the
Cockpit.

A credential a capability reads from the environment is never a value on any surface: what is
reported is its status — configured, missing, invalid, unavailable — and never the thing
itself.

## Failure behaviour

A reviewer refuses a name-matching redactor, a second redaction site, and an input type that
carries a secret without declaring it. The unit tests of `execution::redact` fail if a
declared marker stops being honoured, if a name that merely looks like a secret is destroyed,
or if a nested, referenced or mapped value escapes; `tests/executions.rs` asserts that a
redacted input reaches no snapshot, event or page in the clear.

## Verification

`apps/majordomus-cli/src/execution/redact.rs` and its tests, `apps/majordomus-cli/tests/executions.rs`,
and review. `docs/EXECUTIONS.md` states the contract for anyone adding a capability that
takes one.
{% endraw %}
