# GOVERNANCE IMPROVEMENT LOOP (mandatory after EACH slice)

1. Collect actual failure modes: drift, duplication, missing permissions, event bugs, UI inconsistency, late CI or expensive repeated work.
2. Search current .ai/rules/doctrines/ADRs validators for coverage; prefer amending/merging one existing canonical rule.
3. Classify proposal: implementation fix, test-only fix, rule tightening, documentation clarification, or new invariant. No new rule unless an existing invariant cannot express it cleanly.
4. Create failing negative fixture proving the defect, demonstrate gate fails before fixing. Implement narrow guard. Demonstrate fixture now rejected while positive examples still pass.
5. Recompute derived docs/capability surfaces, ensure policy doesn't require duplicated metadata or create heavy startup cost. Record backwards compatibility and migrations.
6. Audit rule economics: maintenance cost, token/context cost, CI runtime cost, false positives and actual recurrence prevented. Prune redundant rules only with equivalence proof, migration and green tests.
7. Record evidence: rule IDs, validator IDs, before/after test result and affected surfaces. For high-impact normative changes, follow existing ADR process.
8. NEVER auto-weaken security, provenance, inherited `.ai` rules, required tests or branch protection to improve throughput.
