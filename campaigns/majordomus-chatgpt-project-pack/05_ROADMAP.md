# Majordomus Roadmap

This file captures the roadmap currently established for the project.

Each step is gated by the previous one being real. A roadmap item becomes a guaranteed claim only when behavioral tests prove it.

| Version | Adds |
|---|---|
| 0.1 | one policy, four profiles, durable state, the eight commands, doctor with wiring reconciliation, projections for four providers, behavioral tests |
| 0.2 | opt-in runtime adapters: read-size clamp, output condensation, subagent budget, with limits derived from the profile |
| 0.3 | execution telemetry, only from providers that expose it honestly |
| 0.4 | cost per accepted outcome, only on measured data |
| 0.5 | routing recommendations derived from 0.4 |
| 1.0 | shared policy across repositories and workers |

## Roadmap discipline

- Do not describe later phases as current capability.
- Do not move telemetry-dependent features earlier by substituting fabricated estimates.
- Do not implement routing recommendations before measured outcome/cost data exists.
- Do not call a roadmap item complete until acceptance behavior is proven.
- Keep implementation order aligned with dependency order unless repository evidence justifies a deliberate change.

## Current boundary

The v0.1 foundation must establish trustworthy policy, durable state, CLI behavior, diagnostics/reconciliation, provider projections, and behavioral tests.

Everything else depends on that foundation being real rather than cosmetically present.
