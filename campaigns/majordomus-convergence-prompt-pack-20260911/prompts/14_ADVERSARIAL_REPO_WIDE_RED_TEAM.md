# Stage 14 — Adversarial Repository-Wide Red Team

Assume the implementation team accidentally taught the tests to agree with itself. Try to falsify every important claim.

## Method

Create temporary mutation fixtures/branches/patches that violate one invariant at a time. The expected canonical gate must fail for the correct reason. Revert each mutation after verification.

At minimum attempt:

1. add a public mutating shell command with no capability backing,
2. add a capability handler not exposed/registered through canonical metadata,
3. add a blocking rule with no validator,
4. add a validator that no canonical gate invokes,
5. add a rule whose linked test does not actually fail when validator is broken,
6. add changed production code/branch with no test coverage,
7. add a consumer-specific sort and nondeterministic HashMap/filesystem ordering,
8. add an unpinned shell `sort`,
9. add a second production lease parser/reader,
10. add a provider to metadata without lifecycle capability declaration,
11. create Cockpit nav/route enum mismatch,
12. add a frontend-only command registry entry,
13. hand-edit a generated file,
14. mark an unbuilt feature claim guaranteed,
15. make a guaranteed claim's evidence stale/red,
16. leave a required change dirty/unpushed/unintegrated and run session finish/doctor,
17. create a new canonical entity and verify zero-registration propagation,
18. reorder source enumeration and run repeated outputs,
19. start concurrent runtime/server instances and attempt lease conflict,
20. attempt unsafe remote peer mutation if distributed stage is present.

## Output

Produce a table:

| Mutation | Expected guard | Actual guard | Correct diagnostic | Pass? |
|---|---|---|---|---|

Any mutation that slips through is a real bug. Fix the architecture/gate and add a permanent regression test before continuing.

## Acceptance

All core invariants have demonstrated negative proof: violating them causes an automatic, actionable failure.
