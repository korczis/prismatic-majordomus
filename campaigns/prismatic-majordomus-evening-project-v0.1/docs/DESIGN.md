# Design

## Product thesis

Majordomus is a lightweight supervisory layer around AI-assisted work. Its portable artifact is `.majordomo/`.

## Design goals

Minimum sufficient context, durable externalized state, explicit scope, profile-based reasoning discipline, concise outputs, deterministic validation where possible, handovers without transcripts, explicit completion criteria.

## Non-goals

No model invocation, dynamic routing, centralized telemetry, daemon, database, web app, or autonomous orchestration in v0.1.

## Architectural rule

`.majordomo/` must be self-contained and must not require private source repositories or infrastructure.
