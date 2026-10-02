# Testing and Guarantees

## Principle

Majordomus favors executable evidence over claims.

A guarantee is not a sentence in a README. A guarantee is behavior that is demonstrably enforced.

## Test hierarchy

Prefer, in order of evidence strength for user-visible guarantees:

1. black-box behavioral tests,
2. integration tests,
3. contract tests,
4. focused unit tests.

Mocks are useful only when they do not hide the boundary being claimed as tested.

## What tests should prove

Where applicable, tests should cover:

- documented guarantees,
- state transitions,
- persistence across restart,
- idempotency,
- failure behavior,
- partial failure,
- reconciliation,
- projection correctness,
- stale projection detection,
- CLI exit status and output,
- provider adapter contracts,
- unavailable provider capabilities,
- configuration versus actual wiring.

## Regression rule

Bug fixes should normally include a regression test reproducing the original failure.

## Diagnostics rule

A doctor/diagnostic command must test meaningful conditions.

Useful distinctions include:

- configured,
- reachable,
- wired,
- active,
- healthy,
- reconciled.

A generic PASS should only summarize actual checks, never replace them.

## Guarantees matrix

Documentation should maintain a clear distinction between:

- Guaranteed: backed by executable evidence.
- Implemented: code exists, but guarantee may be incomplete.
- Experimental: behavior exists but is unstable or intentionally not guaranteed.
- Planned: roadmap only.
- Unsupported: explicitly outside the contract.

Roadmap promotion rule:

planned -> implemented -> behaviorally verified -> guaranteed

Do not skip stages by editing prose.
