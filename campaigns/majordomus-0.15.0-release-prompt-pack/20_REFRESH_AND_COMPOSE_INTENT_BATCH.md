# Milestone 2 — Refresh and compose the intent batch

Goal: one canonical candidate containing current master plus intended 0.15.0 behavior without reverting later fixes.

Use current integration policy/ADRs. Do not choose rebase/merge style by habit.

Required:

1. Update refs and verify canonical worktree.
2. Re-run peer/collision checks.
3. Bring current master into candidate using canonical batch mechanism.
4. Resolve conflicts semantically:
   - authored source beats derived projection;
   - preserve later master fixes;
   - preserve intent semantics;
   - delete superseded duplicates;
   - regenerate generated output.
5. Guard explicitly against reverting post-candidate master work, including newer plan/session cleanup and temp-file cleanup if still relevant.
6. Build correct executable if generation requires it.
7. Run canonical derive under lock, then derive-check.
8. Inspect diff against master and working-tree diff.
9. Run focused intent/conflict tests.
10. If conflict reveals an uncovered semantic defect, add regression test for invariant, not conflict shape.

Acceptance:
- candidate contains current master;
- no unresolved conflict;
- no generated drift;
- no known newer-master regression;
- focused tests green;
- candidate clean/pushed as policy expects.

Report old/new refs, conflicts and decisions, generated files, focused proof.
