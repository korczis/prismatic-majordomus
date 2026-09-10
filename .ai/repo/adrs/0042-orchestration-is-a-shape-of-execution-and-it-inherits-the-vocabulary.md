---
schema: adr/v1
id: adr-0042
kind: adr
title: Orchestration is a shape of execution, and it inherits the vocabulary
status: accepted
date: 2026-09-11
tags:
  - architecture
  - capabilities
  - rust
  - agents
  - cockpit
related:
  - file:.ai/repo/adrs/0024-an-orchestrator-is-a-provider-only-at-the-bootstrap-level-an.md
  - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
  - file:.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md
  - file:.ai/repo/adrs/0002-canonical-capability-registry.md
  - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
  - rule:project.no-new-nouns
  - rule:project.interfaces-are-projections
  - rule:project.no-network-no-eval
  - file:docs/CONCEPTS.md
  - file:apps/majordomus-cli/src/providers.rs
  - file:apps/majordomus-cli/src/model.rs
  - file:apps/majordomus-cli/src/execution/mod.rs
  - file:share/schemas/majordomus/session-record/session-record.v1.schema.json
provenance:
  origin: authored
---

# 42. Orchestration is a shape of execution, and it inherits the vocabulary

## Context

A prompt pack arrived asking for a runtime orchestration control plane: a provider
registry with model capability metadata, context-window and cost and latency fields, a
model router that selects among them, escalation on epistemic uncertainty, cross-model
handover, and a Cockpit that renders all of it. Several sessions began working in that
territory within the same hour, which is the reason this ADR exists before any type does.

Three pieces of repository evidence decide the design, and two of them contradict the
pack's vocabulary outright.

**`provider` is taken, and it means the opposite thing.** `apps/majordomus-cli/src/providers.rs`
is the provider *projection*: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` and whatever else the
policy's `projections[]` declares, rendered one-way from the policy and the templates. A
provider here is a worker's instruction surface, not a vendor of inference. ADR 32 already
settled the same collision from the other direction, ruling that an external workspace is
not a provider. A second meaning for the word would make `majordomus providers` ambiguous
on every surface that speaks it.

**`model` is taken, and it means the object domain.** `apps/majordomus-cli/src/model.rs` is
`Object`, `Diagnostic`, `Severity` — what an object is, where it came from, and what was
wrong with what could not become one. Every projection consumes those types. "Model" in
this repository is never an inference engine.

**A catalogue of workers is already refused.** `project.no-new-nouns` is `class: blocking`,
and `docs/CONCEPTS.md` states the reason in its own words: *"No agent, persona, role, tier,
or registry or catalogue of workers. A supervisory tool that adds nouns becomes the thing
it supervises. The only actor Majordomus knows is `owner`, a free-form string on the task
record."* A provider registry carrying per-model capability, context-window, cost and
latency metadata is precisely a catalogue of workers. It is not a near miss.

What the repository does instead is already built. `session-record.v1` carries `worker`:
*"What did the work, as it identified itself: a tool and a model, never a person's name."*
Which tool and which model did a piece of work is **self-reported evidence on an immutable
record**, not a table the tool maintains and must keep true. And `src/execution` already
holds the machinery an orchestration plane would otherwise reinvent: an identity, a
lifecycle, a typed event stream, a cancellation flag, redaction, a store that fans out, and
projections to HTTP, MCP, the command line and the Cockpit's WebSocket — with ADR 33
stating that this is a watched capability call and not a second registry.

## Decision

**1. Orchestration reuses neither `provider` nor `model`.** Both nouns keep their current
meanings. No orchestration type, field, capability, route, CLI word or Cockpit label may
introduce a second sense of either. Where the inference side genuinely needs a word, it is
`worker`, which the session record already defines, already scopes to "a tool and a model"
and already treats as self-reported.

**2. Majordomus gains no catalogue of models, vendors, costs or capabilities.** Nothing in
the tool enumerates what models exist or what they can do. Such a table is unmaintainable
by construction — it goes stale the day a vendor ships — and `project.no-new-nouns` refuses
it. Which worker did the work is recorded after the fact, from the worker's own claim, on
the record that already has the field.

**3. Routing is therefore recorded, not planned.** A router that selects among models needs
the catalogue rule 2 forbids, so the tool does not select. What it does is make a choice
already taken **inspectable**: the worker that ran, the episode it belonged to, the
evidence it produced, and — where a worker chooses to report one — the reason it escalated
or handed over, as a typed event on the execution it belongs to. This is the pack's
"explainable routing decision" without the registry underneath it.

**4. Orchestration state is a shape of execution, not a parallel snapshot.** Anything the
Cockpit needs about work in flight extends `src/execution` — its events, its store, its
existing projections. No `OrchestrationSnapshot`, no second lifecycle, no second event
contract. The pack names that type; ADR 33 and rule `project.interfaces-are-projections`
outrank the name.

**5. Any noun that survives this enters `docs/CONCEPTS.md`.** That file is the whole
vocabulary, and `test/cases/28_no_hardcoded_values.sh` checks that every term in it is one
the tool actually uses. A concept absent from CONCEPTS.md is not a concept.

## Alternatives considered

**Introduce `provider` as an overloaded term, disambiguated by context.** Rejected. The
word already appears in the policy, in `projections[]`, in `share/providers.yaml`, in the
generated `AGENTS.md`/`CLAUDE.md` stamps and in ADR 24 and ADR 32. Overloading it makes
every one of those surfaces require a reader to know which sense is meant.

**Keep a small, hand-maintained model catalogue, marked advisory.** Rejected. Advisory
tables are the failure mode the repository has already catalogued: something reads them as
truth, and nothing makes them true. An advisory catalogue is also still a catalogue, so
`no-new-nouns` refuses it regardless of the label.

**Derive the catalogue at run time from each vendor's API.** Rejected here, though it is the
only version that could stay true. `project.no-network-no-eval` is `class: blocking` and
states that `bin/`, `lib/`, `share/` and `test/` contain "no network client but the one
declared exception SECURITY.md names", so the shell tool cannot do it at all and the
executable would need a declared exception to. It would also make the tool's answer about a
repository depend on credentials it has no other reason to hold.

**Build `OrchestrationSnapshot` as the pack specifies and reconcile later.** Rejected. A
second lifecycle beside `src/execution` is the "temporary implementation that becomes a
second architecture" the pack's own contract lists as forbidden.

## Consequences

The pack's phases are re-scoped rather than executed as written, which the pack's contract
explicitly provides for: *"If repository evidence contradicts this prompt, preserve the
invariant and adapt the implementation to reality. Document the divergence."*

- Its provider-registry and model-capability-metadata work is **declined**, with the
  reasons above. This is the largest single reduction in the pack's scope.
- Its context compiler, token accounting and provenance work stands, and none of it needs
  either contested noun.
- Its routing, escalation and cross-model handover work becomes recording and rendering
  what a worker reports, on the execution and the session record.
- Its Cockpit phase renders `src/execution` plus session evidence, and owns no semantics of
  its own, per ADR 12.

Cost: a worker cannot ask Majordomus which model to use, and never will be able to. That is
the intended trade — the tool supervises work and keeps continuity across workers; it does
not become a broker for them.

Benefit: the three sessions presently inside this territory have one vocabulary and one
extension point, decided from evidence, before any of them names a type. That was the
actual risk this ADR was written against.
