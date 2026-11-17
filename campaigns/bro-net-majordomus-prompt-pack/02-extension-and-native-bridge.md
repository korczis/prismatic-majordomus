# Prompt 02 — Browser Extension and Local Bridge


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
Create the minimal browser runtime that can connect a real browser to Majordomus.

Prefer a narrow architecture:
browser extension
  -> authenticated local bridge/native messaging/WebSocket mechanism
  -> Majordomus browser peer adapter
  -> mesh

Inspect repository and choose the integration style that best fits existing runtime/server architecture.

Implement:
- extension manifest and bootstrap
- explicit user-visible connection state
- secure local pairing/registration
- heartbeat
- capability advertisement
- minimal task receiver
- minimal result sender
- disconnect/reconnect
- deterministic extension build
- local development workflow

Security:
- default deny
- restrict origins/permissions
- do not request `<all_urls>` unless technically required and justified
- never transmit cookies/auth tokens unless a future capability explicitly allows it
- never log page secrets
- protect local bridge from arbitrary unauthenticated clients
- make pairing revocable
- make active execution visible to the user

Start with a tiny capability set, ideally:
- navigate/read current URL
- extract sanitized page metadata/text
- screenshot if supported cleanly

Do not implement form submission or arbitrary JS execution unless already required by the existing Majordomus task model.

Tests:
- extension unit tests where practical
- bridge protocol tests
- invalid pairing
- unauthorized client
- reconnect
- origin scope
- sanitized extraction
- browser runtime unavailable
- multiple tabs
- deterministic manifest/build

Acceptance criteria:
- a real browser can become an online Majordomus peer
- registration and heartbeat work end to end
- at least one safe capability executes
- connection can be revoked
- runtime is observable
- no hidden browsing telemetry is collected
