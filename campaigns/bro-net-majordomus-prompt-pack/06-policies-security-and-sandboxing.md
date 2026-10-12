# Prompt 06 — Browser Policies, Permissions and Sandboxing


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
Turn browser execution from "it works" into "it is controlled".

Threat-model the actual implementation.

Cover:
- local pairing/authentication
- peer impersonation
- malicious webpage content
- prompt injection through page text
- credential/cookie leakage
- cross-origin access
- arbitrary JS execution
- data exfiltration
- excessive permissions
- stale sessions
- replayed tasks
- malicious/compromised extension
- task origin spoofing
- screenshot sensitivity
- persistence of sensitive extracted content

Implement policy controls using existing Majordomus rules/doctrines/policy framework.

Required principles:
- default deny
- least privilege
- explicit origin scope
- capability-level authorization
- inspectable permissions
- revocable trust
- user-visible active execution
- secret redaction
- structured audit trail
- no hidden background browsing
- no behavioral tracking by default
- no CAPTCHA-solving feature
- no silent distributed compute

Where browser sandboxing cannot guarantee something, document the limitation precisely instead of pretending.

Add security-focused tests:
- unauthorized origin
- unauthorized capability
- forged task
- replay
- peer identity mismatch
- sensitive headers/cookies absent from logs/results
- prompt-injection content remains untrusted data
- revoked peer cannot execute
- policy changes propagate correctly

Acceptance criteria:
- security model is explicit, tested and enforced
- policies are not decorative docs
- dangerous capabilities are absent or gated
- threat model is documented
