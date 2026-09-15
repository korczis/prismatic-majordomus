# Global execution contract

Before changing code, perform the repository's own bootstrap exactly as the live `AGENTS.md` and `.ai/README.md` require.

Non-negotiable constraints:

1. Read and obey live rules and their dependencies. Do not trust filenames alone.
2. Resolve path-specific context before changing files.
3. Use the canonical branch/worktree topology. Do not work on a feature branch in the trunk checkout.
4. Every substantial change must be attached to the repository's issue/milestone lifecycle if that lifecycle is enabled by the live policy.
5. Existing architecture outranks this prompt. Adapt this requested intent to the repository's current canonical mechanisms rather than creating parallel machinery.
6. No manual duplicate registration of a provider, model, capability, route, MCP tool, OpenAPI operation, Cockpit page, doc entry, benchmark target or skill projection where it can be derived.
7. Do not silently weaken a rule or bypass a gate to make the change pass.
8. Do not expose secrets or raw credentials in any output, diagnostics, snapshots, fixtures, generated docs or browser state.
9. New user-visible claims require executable evidence through the project's use-case/test system.
10. Update docs/ADRs/rules in the same change when semantics change.
11. Prefer strongly typed Rust models plus `schemars`/existing schema machinery over untyped JSON maps.
12. Preserve deterministic generation.
13. No network activity on repository-open or other hot paths unless an explicit operation requests it.
14. Provider discovery failure must degrade/refuse according to explicit semantics. Never fabricate model capabilities.
15. Treat external provider data as untrusted input.
16. Do not persist raw provider responses by default merely because they are available.
17. Keep provider-specific protocol vocabulary below the adapter boundary unless a provider-specific diagnostic explicitly needs to surface it.
18. Finish with focused tests, cross-projection parity tests, generation checks, use-case coverage, docs checks, lints and the repository's normal `finish` evidence.


# Mission 08 — Cockpit provider control plane, live runs and operational diagnostics

Extend Cockpit as a projection, respecting ADR 0012.

The Cockpit must not gain a provider repository, model catalog, routing logic or secret store in JavaScript.

## Pages/views

Derive navigation and content from capability/object/module metadata.

Add useful human views:

### Providers
- configured providers;
- credential state: configured/missing/error only;
- endpoint identity without secrets;
- live/cache/offline status;
- provider health;
- capability evidence.

### Models
- discovered models;
- provider;
- normalized capabilities;
- capability provenance;
- selection inspector;
- stale/live marker.

### Runs
- recent local runs if run summaries are retained;
- live event stream where architecture supports it;
- tool-call timeline;
- usage;
- terminal status;
- source task/session links.

### Hosts
- Codex/ChatGPT/etc detection;
- MCP configuration status;
- skill projection status;
- diagnostics.

### Ideas / Inbox
If Mission 06 introduced the concept:
- triage list;
- source/provenance;
- relation graph;
- promotion action using the canonical capability.

## Mutation UX

Any write action:
- invokes the canonical capability;
- displays exact scope/effect before execution when meaningful;
- provides CSRF/auth protection if HTTP mutation exists;
- never relies on browser-only validation;
- shows returned canonical result.

## Streaming

If the server now supports an event stream, progressively enhance the page. Complete non-JS HTML must still work.

Do not introduce a SPA or duplicate backend state.

## Graph integration

Extend the composed graph using existing relation machinery:
- provider → models;
- run → provider/model/session/task;
- idea → source/issue/ADR/rule/skill after promotion.

Only add relations where authoritative references exist.

## Diagnostics

`health.report` or equivalent should delegate to provider/host engines rather than reimplementing checks.

Add reproducible diagnostic commands.

## Security

Run browser/CSP tests.
Prove credentials/API keys never appear in:
- HTML;
- embedded JSON;
- JS state;
- error pages;
- logs;
- screenshots/probes;
- OpenAPI examples.
