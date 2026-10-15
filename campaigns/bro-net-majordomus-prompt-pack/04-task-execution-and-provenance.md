# Prompt 04 — Browser Task Execution and Provenance


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
Implement a canonical browser task/job execution pipeline with evidence and provenance.

Pipeline:
intent/task request
  -> validation/policy
  -> peer selection
  -> dispatch
  -> browser execution
  -> structured result
  -> provenance/evidence
  -> persistence/session context
  -> diagnostics/observability

Implement a narrow safe task set first.

Every task should have:
- canonical id
- requested capability
- target/origin
- requester/session/intent
- peer
- timestamps
- state transitions
- timeout/cancellation
- result/error
- provenance/evidence references

Integrate with existing Majordomus job/task/execution abstractions if present.

Result examples may include:
- page URL/title
- sanitized text extraction
- structured DOM-derived facts
- screenshot artifact reference
- execution metadata

Do not store raw sensitive browser content unnecessarily.
Do not claim provenance that cannot actually be traced.

Tests:
- success
- validation failure
- policy rejection
- timeout
- cancellation
- peer disconnect mid-task
- malformed result
- retry behavior according to existing policy
- provenance completeness
- task/result schema compatibility
- no-secret logging checks where infrastructure exists

Acceptance criteria:
- tasks are durable/observable according to Majordomus conventions
- result provenance is inspectable
- session/context can reference browser evidence
- errors use canonical diagnostics
