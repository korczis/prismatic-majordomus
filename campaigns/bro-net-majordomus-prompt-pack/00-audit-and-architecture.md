# Prompt 00 — Bro-Net / Browser Peer Audit and Architecture


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
Design the smallest credible Bro-Net implementation as a Majordomus browser-peer subsystem.

Do not begin by coding a browser extension. First determine what Majordomus already has that can own:
- peer identity
- mesh membership
- capability advertisement
- task routing
- session/context integration
- provenance
- policy enforcement
- diagnostics
- Cockpit representation
- CLI/API/MCP exposure
- testing and deployment

Read relevant AGENTS.md, .ai/**, .majordomus/**, ADRs, rules, doctrines, schemas, mesh/session/context code, provider adapters, Cockpit, API/OpenAPI, MCP, CLI, tests, deployment docs.

Produce an internal architecture map:
component | current source of truth | reusable? | gap | migration/new code required

Define the target pipeline:

browser runtime
  -> browser peer adapter
  -> canonical Majordomus peer model
  -> mesh registration
  -> capability registry
  -> authorized task execution
  -> provenance-backed result
  -> CLI/API/MCP/Cockpit/docs

Hard constraints:
- Browser must be a peer type, not a separate orchestration universe.
- No speculative "global browser supercomputer".
- No behavioral manipulation.
- No collection of browsing history or user behavioral telemetry by default.
- No CAPTCHA-solving network.
- No background compute beyond explicitly authorized tasks.
- No hidden network activity.
- Permissions must be explicit and inspectable.
- Browser-specific state must not leak secrets into logs or APIs.

Define MVP capability vocabulary, but infer naming from repository conventions. Likely classes include:
- browser.navigate
- browser.extract
- browser.dom.read
- browser.screenshot
- browser.form.inspect
- browser.form.fill
- browser.click
- browser.storage.read/write (scoped)
- browser.tab.list/focus
- browser.health

Do not implement every capability now. Decide the minimal first set.

Acceptance criteria:
- architectural owner for browser peers is identified
- no duplicate mesh/runtime model is introduced
- capability and task abstractions have canonical homes
- security boundaries are explicit
- implementation phases are ordered
- required ADR/rule/doctrine changes are identified according to repository policy
- a concrete migration plan exists
