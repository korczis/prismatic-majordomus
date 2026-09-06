---
schema: moment/v1
id: api-changed-contract-did-not
kind: moment
title: 'The interface changed and its contract document did not'
short_title: 'Contract drift'
hook: 'shipped an interface change whose contract document still described the old one'
summary: 'A contract maintained by hand beside the code it describes goes stale on the first change that forgets it.'
status: stable
severity: high
frequency: common
weight: 250
audiences: [platform-team, enterprise, open-source-maintainer, ai-native-team]
areas: [verification, documentation]
lifecycle: [implementation, review]
tags: [api, openapi, contract, drift, generation]
signals:
  - id: hand-written-spec
    text: 'An interface specification is maintained by hand alongside the code that implements it.'
  - id: spec-behind
    text: 'A published contract describes a version of the interface that no longer exists.'
  - id: two-places-to-edit
    text: 'Adding an operation means editing both the implementation and a separate registry or document.'
examples:
  - id: openapi-by-hand
    audience: platform-team
    title: 'The specification written twice'
    before: 'A field is added to a response type and the specification is not updated, so every generated client is wrong.'
    after: 'The specification is derived from the same typed declaration the implementation uses, and CI refuses a tree in which it is stale.'
  - id: sdk-consumers
    audience: open-source-maintainer
    title: 'Consumers generated from a stale document'
    before: 'Downstream clients are generated from a published contract that drifted two releases ago.'
    after: 'The published document is a build output of the declaration; there is no second place for it to drift from.'
  - id: control-boundary
    audience: enterprise
    title: 'A boundary that was reviewed once'
    before: 'The reviewed interface document and the served interface are two artefacts with no enforced relationship.'
    after: 'One declaration, many projections; the reviewed artefact is regenerated and diffed rather than trusted.'
commands: [doctor, bench]
capabilities: [capabilities.list, capabilities.describe]
responsibilities: [projection, doctor]
claims: [interfaces-are-projections, openapi-inferred, generated-projections-checked, capability-registry, schema-driven-kinds]
doctrines: [project.interfaces-are-projections, project.rust-canonical-declaration, project.derived-files-regenerated, majordomus.projection-integrity]
use_cases: [extend-what-the-executable-serves, serve-the-layer-to-ai-clients, gate-ci-on-the-tool-itself]
related: [generated-artifacts-stale, documented-command-no-longer-works, policy-changed-projection-stale]
aliases: ['stale OpenAPI', 'spec drift', 'contract out of date']
---

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
