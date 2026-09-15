# Prompt 02 — Canonical IDE/Control-Plane Contract

Continue from Prompt 01 and use its verified audit.

## Mission

Define the minimal canonical domain/runtime contracts needed for Cockpit to become a development IDE and monitoring/control surface **without making Cockpit the owner of semantics**.

## Core design rule

The desired architecture is:

```text
repository + git + .ai + canonical capability declarations + runtime state
                               ↓
                    canonical typed registries
                               ↓
                       one execution plane
                               ↓
          query / command / workflow / stream / artifact contracts
                               ↓
 CLI · HTTP · OpenAPI/Swagger · MCP · Cockpit · generated docs/GH Pages
```

Not:

```text
Rust API model
Cockpit model
Swagger model
MCP model
CLI model
docs model
six humans praying they stay synchronized
```

## Required contract review

Inspect existing models before adding anything:

- capability descriptor and exposure;
- execution metadata;
- execution event stream;
- cancellability;
- input/output schemas;
- examples/benchmark cases;
- provenance;
- stability;
- diagnostics;
- repository info;
- worktree topology;
- peer state;
- object model;
- graph model;
- health model.

Extend only where necessary.

## Define first-class IDE concepts only if missing

Possible concepts include:

### Action
A typed operation a human or agent can invoke.

Prefer representing this as an existing capability rather than inventing `IdeAction`.

### Execution
A durable/reloadable representation of a run with:
- id;
- capability/workflow identity;
- actor/peer;
- timestamps;
- lifecycle state;
- progress;
- steps;
- live output;
- diagnostics;
- result/error;
- cancellation semantics;
- provenance;
- parent/child relations if fan-out exists.

### Workflow
If the repository already has `.ai/workflows`, connect those definitions to executable capabilities rather than creating Cockpit workflow JSON.

A workflow should be discoverable from canonical metadata and expose:
- title;
- intent;
- inputs;
- preconditions;
- steps;
- expected outputs;
- applicable rules/doctrines;
- use cases;
- risks;
- execution entrypoint;
- status.

### Artifact
A canonical reference to generated/build/test/log/diff/report output.

Do not store arbitrary huge blobs in UI state. Define typed metadata + retrieval semantics.

### Development context
A typed answer assembling relevant:
- repo identity;
- branch/worktree;
- task/issue/milestone;
- session;
- peer claims;
- rules/doctrines;
- relevant files;
- tests;
- ADRs;
- knowledge;
- git state;
- generated drift;
- runtime status.

This should reuse/extend existing context machinery, not bypass it.

## Transport parity

For every new executable capability:

- decide whether it is query/command/workflow according to existing semantics;
- expose through CLI unless there is a justified exception;
- expose via HTTP if remotely/browser useful;
- derive OpenAPI automatically;
- expose via MCP when agent-useful;
- make Cockpit discover it generically;
- include it in generated docs/reference;
- supply benchmark/use-case inputs as required by repository rules.

No hand-coded Cockpit route/action is allowed merely to invoke a backend operation that should be a capability.

## Projection metadata

If the Cockpit needs UI hints, add **generic typed presentation metadata** only when real value exists, for example:
- category/domain;
- icon semantic token;
- risk/effect class;
- preferred renderer;
- output media/schema;
- confirmation requirement;
- streaming availability.

Do not encode page layouts or CSS in Rust descriptors.

## Acceptance proof

Add contract tests showing that adding one representative new capability/workflow causes it to become discoverable in every allowed projection without editing:
- Cockpit menu/catalogue;
- HTTP registry;
- Swagger;
- MCP registry;
- CLI registry;
- generated reference.

Prove with a disposable test capability or fixture, not rhetoric.
