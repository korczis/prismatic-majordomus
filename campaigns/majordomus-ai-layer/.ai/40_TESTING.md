# Testing and Guarantees

## Principle

A guarantee is not README prose. A guarantee is behavior that is demonstrably enforced.

## Preferred evidence

For user-visible guarantees prefer:

1. black-box behavioral tests,
2. integration tests,
3. contract tests,
4. focused unit tests.

Mocks must not hide the boundary being claimed as tested.

## Tests should cover, where applicable

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

Bug fixes should normally include a regression test reproducing the original failure.

## Diagnostics

Diagnostics should distinguish concepts such as:

- configured,
- reachable,
- wired,
- active,
- healthy,
- reconciled.

A generic PASS may summarize actual checks. It must never replace them.

## Guarantee lifecycle

Use explicit states:

planned -> implemented -> behaviorally verified -> guaranteed

Also use `experimental` and `unsupported` where appropriate.

Do not promote a claim by editing prose alone.
