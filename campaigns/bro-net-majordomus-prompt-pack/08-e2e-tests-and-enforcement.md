# Prompt 08 — End-to-End Tests, Coverage and Enforcement


You are working inside the `prismatic-majordomus` repository.

Non-negotiable execution discipline:
- Inspect the real repository before deciding architecture.
- Reuse existing Majordomus abstractions, registries, rules, doctrines, schemas, adapters, diagnostics, tests, and deployment paths.
- Do not create parallel systems if an existing canonical mechanism can be extended.
- Everything must be typed, deterministic, data-driven, discoverable, testable, documented, and enforced where appropriate.
- No hardcoded duplicate inventories across CLI/API/MCP/Cockpit/docs.
- No claims without repository evidence.
- No "implemented" status unless tests pass and the relevant runtime/deployment surface is verified.
- Preserve existing public interfaces unless a migration is justified and completed.
- Prefer deleting obsolete duplicated machinery over adding another adapter layer.
- Use the repository's real canonical commands for format/build/test/check/deploy. Discover them instead of inventing them.
- At the end, report root cause, architecture, files changed by purpose, tests executed, validation evidence, and remaining debt.


Mission:
Make browser-peer support regression-resistant.

Build a complete test pyramid:
- domain/unit
- protocol/schema
- mesh integration
- bridge integration
- browser/extension integration
- API/MCP/CLI
- Cockpit
- end-to-end
- security/policy
- generated artifact drift

Use the repository's real testing stack.

Create deterministic browser fixtures/mocks where appropriate, but also include at least one real-browser E2E path if CI/runtime allows.

Coverage:
- integrate browser-peer modules into the existing coverage pipeline
- ensure coverage is visible through the same test/coverage UI/docs mechanisms Majordomus uses
- no separate hand-maintained report path

Enforcement:
- required tests run in canonical quality gates
- schema drift fails
- OpenAPI drift fails
- generated docs drift fails
- security-policy regressions fail
- browser peer protocol compatibility fails loudly

Regression scenarios:
- browser peer forgotten from one consumer
- stale peer routed
- revoked peer executes
- duplicated capability inventory
- non-deterministic peer selection
- missing provenance
- secret leaked to logs/result
- extension protocol version mismatch

Acceptance criteria:
- failures are actionable
- CI and local canonical checks cover the feature
- test/coverage evidence is linked from existing Majordomus UI where supported
