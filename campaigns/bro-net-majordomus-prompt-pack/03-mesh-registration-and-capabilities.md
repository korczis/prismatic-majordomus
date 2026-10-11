# Prompt 03 — Mesh Registration, Discovery and Capability Routing


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
Integrate browser peers fully into Majordomus mesh discovery and capability routing.

Requirements:
- browser peers register through canonical mesh mechanisms
- no separate browser peer directory
- capabilities use canonical capability registry/types
- peers are discoverable by capability, health, status, session/context and whatever constraints already exist
- routing is deterministic for equal candidates
- peer loss/staleness propagates through standard diagnostics
- Cockpit/API/MCP/CLI inventories derive from the same source

Implement routing behavior for tasks such as:
- choose online peer advertising required browser capability
- respect explicit peer selection when requested
- reject when no authorized peer exists
- avoid stale/offline peers
- surface why routing failed

Do not add "smart" heuristic routing unless existing Majordomus routing already supports it. Start deterministic and explainable.

Tests:
- one peer
- multiple peers
- capability mismatch
- stale peer
- explicit peer selection
- reconnect
- duplicate capabilities
- deterministic tie breaking
- concurrent registration
- peer removal
- no manual consumer registration after adding a browser peer

Acceptance criteria:
- browser peers appear naturally in mesh
- capability lookup works
- routing is explainable
- no duplicate registry exists
- cross-surface inventories agree
