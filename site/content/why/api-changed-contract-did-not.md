+++
title = "The interface changed and its contract document did not"
description = "A contract maintained by hand beside the code it describes goes stale on the first change that forgets it."
weight = 250
[extra]
id = "api-changed-contract-did-not"
status = "stable"
source = ".ai/repo/why/moments/api-changed-contract-did-not.md"
+++
{% raw %}

## The moment

A response type gains a field. The implementation is correct, the tests pass, and the
published specification still describes last month's shape — so every consumer generated
from it is wrong in a way that will surface in somebody else's codebase.

## Why it happens

The specification is a second, hand-maintained representation of something the code already
states precisely. Two representations of one fact require an act of synchronisation, and
that act is performed by memory. It is skipped the first time somebody is in a hurry, and
after that the document is untrustworthy, so people stop reading it, so it stops being
updated at all.

## Why a better model does not fix it

A worker asked to change the implementation changes the implementation. Asking it to also
update the specification is asking for the synchronisation to be performed by something else
that can forget; the fix is to have one representation.

## What it costs

Broken consumers, whose breakage is discovered by the consumer rather than by the producer.
And the slow loss of the document itself: once a specification has been wrong twice, it is
treated as documentation rather than as a contract.

## What Majordomus does

An operation is declared once, as a typed descriptor beside its implementation, with its
input and output types. Everything external is derived from that declaration: the MCP tool,
the HTTP route, the OpenAPI operation with its schemas and examples, the browsable
reference, the benchmark target and the generated documentation. Adding an operation by
editing a transport registry or a specification file is not possible, because those are
outputs. `generate --check` names every generated file that differs from what the
declaration produces, and CI refuses the tree.

## Before and after

```text
before   handler.rs (new field)  +  openapi.yaml (hand-written, unchanged)

after    $ majordomus generate --check
         stale: docs/generated/openapi.json (differs)
                [reproduce: majordomus generate]
```

## What it does not do

It does not review an interface design, and it does not decide whether a change is
breaking. It removes the possibility of the description and the behaviour being maintained
separately.
{% endraw %}
