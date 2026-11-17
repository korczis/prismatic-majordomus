# 07 — `.ai/` GOVERNANCE, GENERATED PROJECTIONS AND NON-REGRESSION

Use the exact current `.ai` layout, schema, rules, doctrines, profiles, scopes, context ancestry, claim evidence and generation commands. Do not hand-author shadow configuration or dilute blocking rules to achieve green CI. Extend the canonical architecture for genuinely general reusable invariants:

1. **Canonical UI projections:** no independently maintained domain collection, stage, status or relationship graph in JS/templates; frontend derives from typed backend.
2. **Capability invocation parity:** UI/API/CLI/MCP use one handler/policy; no unregistered mutation endpoints.
3. **Evidence-gated completion:** completion, release and deployment badges backed by current verifiable facts.
4. **Execution observability:** stable IDs, recoverable state, honest terminal outcomes and documented resume semantics.
5. **Effect authorization:** scope, actor, idempotency, concurrency, privilege and remote side effects enforced on the server.
6. **No generated drift:** docs/site/OpenAPI/help/bootstrap and schema projections reproducible and checked in CI.
7. **Deterministic presentations:** stable semantic default ordering and stable tie-breakers, independent of filesystem/hash/concurrency order.
8. **Performance budgets:** bounded startup, rendering, event handling and input responsiveness with monitored measurements.
9. **Mobile accessibility:** navigation/drawers, touch, keyboard, screen-reader and important contrast checks.

Choose whether each becomes an actual rule, doctrine, typed validator, test, CI gate, claim or ADR under *existing* repository conventions. No decorative Markdown-only enforcement. Make diagnostics explain: failed invariant, owner, offending file/operation, expected canonical source and exact remediation.

Test pyramid: Rust unit/property tests; schema and malformed-input tests; domain fixtures; canonical ordering permutation tests; cross-interface contract tests; live execution fault injection; GitHub adapter mock tests; browser E2E on mobile/desktop; security and authz/CSRF/XSS tests; accessibility; smoke; performance regression. Explicit negative tests for policy bypass. Preserve established `test/run.sh`, `just validate`, `just generate --check`, `doctor`, build/check entrypoints where current checkout provides them. Never invent a fake command in CI; discover commands first.

Dogfood by implementing this feature through the same intent→issue→scope→execution→test→evidence chain to the extent currently supported. If a capability cannot dogfood itself, record the exact architectural gap and add the narrowest test/bridge needed. Test adding a sample capability and object link: all relevant derived consumers must discover it without manual registration. Run a simulated failure proving CI/gate catches drift, then restore clean state.

Check for changes outside claimed task scope, leaked secrets, stale generated files, orphan docs claims, unsupported feature maturity and duplicate models. Re-run against a clean checkout/fixture, not only developer's warm workspace. Report exact commands, exit codes, key evidence and unresolved failures. Do not declare readiness on stale evidence.
