# Stage 06 — Canonical Ordering Zero Debt

Finish the deterministic ordering migration. Do not leave a growing grep-based debt counter as permanent architecture.

## Mission

Establish one canonical presentation-order abstraction for machine-discovered collections and prove every public/default projection uses it.

## Tasks

1. Enumerate all current Rust and shell sort/order sites.
2. Classify each as:
   - canonical presentation ordering,
   - domain-semantic ordering,
   - algorithmic/internal ordering,
   - serialization/protocol-required ordering,
   - accidental ordering.
3. Move presentation ordering to the canonical order module/trait/key already established by the repository ADR.
4. For legitimate non-presentation sorts, make the checker understand their category structurally or through a narrow typed allowlist with rationale. Avoid a giant line-number suppression file.
5. Pin shell locale (`LC_ALL=C` or repository-standard deterministic equivalent) where shell sort is genuinely necessary; preferably move canonical presentation logic out of shell.
6. Ensure maps/filesystem/globs/concurrent tasks cannot leak iteration order into public output.
7. Define natural ordering, Unicode/case semantics, group rank and canonical ID tie-breaker in one place.
8. Ensure API/MCP/CLI/Cockpit/docs/generated indexes receive the canonical order without re-sorting independently unless the user explicitly asks for alternate sort.
9. Add property/permutation tests and repeated-process byte-stability tests.
10. Remove/shrink the historical order baseline. If every legitimate sort is classified semantically, retire the coarse count baseline.

## Adversarial tests

- randomize insertion order,
- randomize filesystem creation order,
- use map/hash containers,
- complete discovery futures/tasks in varied order,
- run under differing locale environment,
- use duplicate display labels with distinct IDs,
- numeric suffixes (`x-2`, `x-10`).

## Acceptance

- `order-check` green without increasing accepted debt.
- No user-facing default order is accidental.
- No frontend/docs consumer owns a shadow canonical order.
- The checker distinguishes legitimate internal sorts from presentation debt without brittle line-number exceptions.
- Cross-surface sequence/group parity tests are green.
