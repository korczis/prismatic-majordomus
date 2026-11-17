# Milestone 10 — Rules, schemas, tests, docs and enforcement

Goal: make the architecture impossible to accidentally bypass.

## Rules/doctrines

Encode, in the repository's canonical rule system, at least these invariants where appropriate:

1. intent-governed work starts with a resolved intent binding or typed exemption;
2. a plan cannot execute without current opposition;
3. criterion/intent satisfaction is derived from canonical evidence;
4. completion cannot independently declare intent satisfied;
5. GitHub status does not constitute criterion evidence;
6. consumer surfaces must not reimplement intent semantics;
7. new intent entities participate in projections by discovery/derivation;
8. stale evidence/opposition is not current proof.

Do not create prose-only rules with no enforcement.

## Schema

Ensure typed schema covers all new records:
- session binding,
- opposition result,
- satisfaction projection if serialized,
- desired outcome/draft/revision if added,
- reconciliation result where persisted/serialized.

Generate schemas from canonical types where repository architecture supports it.

## Mutation/wiring proof

For each blocking rule, deliberately break the chain in a fixture and prove:
- doctor/check detects missing validator/wiring,
- command exits non-zero,
- CI runs the proof.

## Property/determinism tests

Add tests for:
- shuffled evidence order,
- shuffled issue enumeration,
- concurrent discovery order,
- repeated run stability,
- stable serialization where guaranteed.

## Cross-surface tests

One canonical fixture should be asserted through:
- domain evaluator,
- CLI JSON,
- API,
- MCP,
- Cockpit backend payload,
- generated docs model where relevant.

## Documentation

Update:
- intent architecture,
- session lifecycle,
- completion semantics,
- opposition,
- evidence/satisfaction,
- GitHub reconciliation,
- Cockpit,
- desired-outcome flow,
- extension guide,
- troubleshooting.

Rewrite the old "Partly built" section so only genuinely unfinished items remain.

Do not manually maintain lists derivable from registries.

## ADR

Use repository ADR policy.
Create/update ADR only if the decisions qualify.

## Acceptance criteria

The new architecture survives:
- broken wiring,
- stale records,
- consumer drift,
- source enumeration changes,
- legacy fixtures.

And documentation describes reality, not aspiration.
