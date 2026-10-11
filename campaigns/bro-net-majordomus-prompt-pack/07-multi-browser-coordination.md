# Prompt 07 — Multi-Browser Coordination


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
Prove the original Bro-Net idea at a practical scale: multiple browser peers can cooperate under Majordomus without creating a new orchestration framework.

Implement one or more bounded workflows demonstrating:
- task fan-out across multiple browser peers
- capability-based assignment
- aggregation of structured results
- provenance retained per peer/result
- partial failure handling
- deterministic aggregation
- cancellation
- observable coordination in Cockpit

Good demo classes:
- distributed page comparison/research
- same task across multiple browsers/origins
- parallel extraction from a bounded URL set

Do NOT implement:
- global public compute
- behavioral alignment
- passive user surveillance
- AI training from idle browsers
- CAPTCHA solving

Reuse existing mesh/session/wave/plan abstractions if present.

Tests:
- 2+ peers
- one peer fails
- slow peer
- duplicate result
- cancellation
- deterministic aggregation
- provenance per result
- no capability mismatch
- concurrency limits

Acceptance criteria:
- genuine multi-peer browser workflow exists
- orchestration reuses Majordomus
- results are explainable and testable
- no browser-specific orchestration island
