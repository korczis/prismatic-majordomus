# Milestone 5 — Full validation and release readiness

Goal: zero unexplained red gates on the composed candidate.

Use focused checks for diagnosis, then canonical aggregate/binding validation.

Cover applicable current mechanisms:

- doctor, context/rules/doctrine/plan/intent validation;
- task/session check and finish-check;
- cargo fmt/build/test/clippy -D warnings/doctests;
- portable shell/shellcheck/relevant shell cases;
- all intent verdict/freshness/preflight/coverage/drift cases;
- API/OpenAPI/schema tests;
- MCP tests;
- Cockpit tests;
- generated artifact drift;
- site/docs/claim/reference checks;
- use-case impact/coverage for public changes;
- differential coverage;
- release tests;
- release analyze/explain/json;
- version-matches-surface gate.

Every failure must be classified:
- RELEASE REGRESSION
- PRE-EXISTING ON CURRENT MASTER
- ENVIRONMENTAL WITH PROOF
- REPRODUCIBLE FLAKE WITH EVIDENCE
- POLICY BLOCKER

"Unrelated" without proving master fails is forbidden.

Acceptance: zero unexplained binding failures, zero generated drift, zero scope violations, stable release analysis, clean/pushed candidate, blocker count zero.

Produce exact command/result ledger.
