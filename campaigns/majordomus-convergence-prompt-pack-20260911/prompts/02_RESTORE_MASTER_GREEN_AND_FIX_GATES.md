# Stage 02 — Restore Trustworthy Green Master and Fix the Gates

Before a migration, the measuring instruments must work.

## Mission

Fix current regressions/false positives in canonical gates without weakening the intended invariants.

Reproduce especially, if still present:

- canonical ordering debt growth,
- unpinned shell collation,
- `lease-reader-check` detecting itself or otherwise producing false positives,
- Cockpit nav/area inventory drift,
- generated drift or stale docs identified in stage 01.

## Rules

1. Do not raise debt baselines to make red green.
2. Do not add broad grep exclusions such as “ignore this script” unless the check structurally proves why the ignored reference is not an independent reader/parser.
3. Every gate bug gets a regression fixture/test.
4. Every real repository violation gets fixed at root cause.
5. Preserve exact deterministic semantics across Linux/macOS where the project supports both.

## Lease reader requirement

If the existing checker uses textual pattern matching that cannot distinguish:

```text
canonical parser implementation
checker mentioning schema identifiers
docs/tests/fixtures
independent production parser
```

replace/refactor the check into a more semantic inventory. The checker should detect a second production lease parser/reader, not merely the spelling of the lease file.

Add a mutation fixture proving a deliberately added second parser fails the gate while the checker itself does not.

## Ordering immediate requirement

Classify current newly detected sort sites and fix actual presentation-order violations. Pin shell sorting with deterministic locale where sorting is legitimately shell-local. Do not complete the entire historical migration yet; stage 06 owns zero-debt convergence. This stage must at least return the ratchet to non-regressed truthful state.

## Cockpit consistency

If areas/routes/docs/nav diverge, derive from one typed inventory or add hard contract tests that make divergence impossible to land. Prefer derivation.

## Acceptance

- All pre-existing core structural gates that are expected green on master are green.
- False-positive checker defects have regression tests.
- No baseline was increased to hide a regression.
- The repository now has trustworthy instruments for later stages.
- Changes are documented where gate semantics changed.
