# Milestone 10: Bind Sessions and Execution to Canonical Intent

## Mission

Make intent a first-class part of session/execution context. The repository already knows what an intent is; now work must execute *under* one when policy requires it.

Do not build a new session subsystem. Extend the existing session envelope/context/start/preflight/execution-plane architecture.

## 1. Audit current session lifecycle

Find the canonical path for:

- session creation/start;
- issue selection/resolution;
- project/workspace resolution;
- context compilation;
- preflight/gates;
- execution start;
- checkpoints/handover/finish;
- serialized session records;
- MCP/API/Cockpit session projections.

Map where intent should be resolved once and propagated rather than repeatedly re-discovered by consumers.

## 2. Define the intent execution envelope

Extend the existing typed session model with the minimum data/reference needed to establish:

- resolved intent identity;
- intent stage/version/revision if the project model supports it;
- issue and milestone relationship;
- criteria served by the current work;
- applicable invariants;
- required evidence obligations;
- critique/opposition disposition required before execution;
- explicit exemption when work is legitimately not intent-bound.

Do not duplicate entire intent records inside session state if stable references plus canonical loading are the repository norm.

Any exemption must be typed, reasoned, policy-defined and auditable. "No intent found, continue anyway" is not an exemption policy.

## 3. Deterministic intent resolution

Implement canonical resolution for common entry modes:

- session started from an issue;
- session started from a milestone/work item;
- session resumed from checkpoint/handover;
- session explicitly names an intent;
- maintenance/emergency work allowed by policy exemption.

Ambiguity must fail loudly with actionable diagnostics. Do not silently pick the first intent.

## 4. Intent-aware preflight

Integrate the existing `intent preflight` semantics into the canonical session/execution preflight so that work cannot bypass them by using another entry command.

At minimum reject:

- issue has no required intent relationship;
- issue claims criteria outside its intent;
- referenced intent/criterion does not resolve;
- intent is in a stage that forbids execution;
- required critique/opposition disposition is missing or stale;
- current work would violate a declared invariant;
- intent-bound work has no evidence obligations when criteria require evidence.

Reuse existing diagnostic types.

## 5. Context compilation

Ensure the compiled context presented to workers includes a concise canonical intent section derived from the typed model:

- why: statement/outcome;
- invariants;
- criteria relevant to this issue/session;
- current satisfaction/evidence state;
- plan/milestone/dependency context;
- critique/opposition result;
- known gaps and blockers.

Avoid dumping the full project model if the context compiler already supports relevance selection/compression.

## 6. Persistence and resume

A resumed session must preserve/re-resolve intent safely.

Detect stale context when:

- intent changed since checkpoint;
- criteria changed;
- issue mapping changed;
- critique became invalidated;
- completion/evidence state changed materially.

Use the repository's existing staleness/divergence mechanisms if available.

## 7. Surface projection

Expose intent linkage through existing structured session status/API/MCP models where appropriate. Do not invent UI here beyond minimal existing session projections; the full Cockpit view is a later milestone.

## 8. Tests

Add tests for at least:

- issue → intent → criteria resolution;
- explicit intent start;
- ambiguous intent failure;
- missing intent failure when required;
- typed exemption success/failure;
- critique missing/stale blocks start;
- session record round-trip;
- resume detects intent drift;
- context includes relevant criteria/invariants;
- execution entry cannot bypass intent preflight;
- legacy non-intent work follows deliberate compatibility policy.

## 9. Documentation and enforcement

Update the canonical session/execution documentation and governing rule/doctrine if repository policy says this invariant belongs there.

Ensure CI/repository gates exercise the intent-bound session path.

## Acceptance criteria

- session start can deterministically resolve its active intent;
- execution context contains intent obligations;
- required intent work cannot begin through any supported execution entry point without valid preflight;
- exemptions are explicit and auditable;
- resume detects relevant intent drift;
- all behavior is tested and documented.
