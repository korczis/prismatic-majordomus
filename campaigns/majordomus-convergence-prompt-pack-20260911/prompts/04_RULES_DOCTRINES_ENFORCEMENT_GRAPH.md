# Stage 04 — Rules/Doctrines Enforcement Graph

Turn “rule exists” into “rule is executable, covered, surfaced and evidenced”.

## Mission

Unify the enforcement semantics of project rules, vendored doctrines, policies and validators without flattening meaningful conceptual distinctions.

First infer the repository's definitions of rule/doctrine/policy. Preserve them. Add a common enforcement graph beneath them.

## Required model

Every core blocking invariant must be machine-traceable through:

```text
rule ID
→ canonical metadata/schema
→ one or more validators
→ canonical command/gate invoking validators
→ failing/passing fixtures
→ unit/integration/E2E tests
→ claims/docs links
→ current evidence/status
→ relevant management surfaces
```

The schema may differ from this sketch, but the graph must be queryable.

## Tasks

1. Inventory all blocking rules and identify human-only/implicit enforcement.
2. Extend canonical metadata/schema so enforcement class is explicit. Distinguish at least absolute machine enforcement, migration ratchet, advisory/human judgment, and generated-contract enforcement if these concepts exist.
3. For every core rule currently marked blocking but human-only, implement a deterministic validator or reclassify it honestly. User requirement is to eliminate human-only blocking for core repository correctness.
4. Ensure validator registration is derived and orphan validators/rules fail doctor/check.
5. Add a canonical `rules verify`/existing-equivalent execution path if needed; do not create redundant command architecture.
6. Make rule status inspectable via structured CLI/API/MCP/Cockpit.
7. Link tests and claims programmatically; generate rule documentation/status pages where the project already derives docs.
8. Add mutation tests:
   - blocking rule with missing validator → fail,
   - validator not invoked by canonical gate → fail,
   - validator always returns pass → caught by negative fixture where practical,
   - missing test linkage → fail,
   - stale generated docs/evidence → fail.

## Ratchets

Where historical migration debt still uses ratchets, encode that truth explicitly. Do not call ratcheted debt an absolute invariant. Core convergence tasks should drive their ratchets to zero and then switch/remove migration mode.

## Acceptance

- Every core blocking rule has executable enforcement.
- Rule → validator → gate → test → evidence traversal works from canonical data.
- No decorative blocking Markdown remains.
- CLI/API/MCP/Cockpit can inspect enforcement status and run/validate where appropriate.
- Documentation derives from the same graph.
- A deliberate broken rule fixture makes CI/local canonical gate fail.
