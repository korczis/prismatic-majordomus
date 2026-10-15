# Prompt 09 — Documentation, Landing Page and Proof Demo


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
Document and demonstrate Bro-Net as a concrete Majordomus feature, not as speculative marketing.

Docs must clearly distinguish:
- implemented
- experimental
- planned
- explicitly out of scope

Explain:
- browser as Majordomus peer
- architecture
- pairing
- capabilities
- task execution
- provenance
- policies/security
- multi-browser coordination
- testing/coverage
- troubleshooting
- extension/developer guide

Landing/demo:
- add an interactive or strongly evidenced Bro-Net/browser-peer section using existing design system
- show real derived data where possible
- link to actual tests/coverage/results
- link to docs/API/MCP/Cockpit routes
- avoid fake counters and fabricated benchmark claims

Create one reproducible demo:
1. start/connect browser peers
2. inspect them
3. dispatch a safe task
4. see result/provenance
5. see task in Cockpit
6. run a multi-peer workflow
7. verify tests

If the public deployment cannot actually expose live local browser peers, document this boundary and use verified recorded/test fixtures rather than pretending.

Acceptance criteria:
- docs match implementation
- claims are evidence-backed
- test links are real
- landing page sells the feature without lying
- extension path for developers is clear
