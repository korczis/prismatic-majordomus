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


# Mission 01 — Lock provider architecture into Majordomus governance

Implement the minimum governance/architecture changes needed so all later provider work is constrained by Majordomus itself.

## Required architecture

Establish or confirm these boundaries:

### ProviderAdapter
Owns provider protocol semantics:
- credentials required;
- endpoint configuration;
- model discovery;
- capability mapping;
- request translation;
- response translation;
- streaming translation;
- provider errors/rate limits;
- provider identifiers.

### HostAdapter
Owns integration with an agent/client environment:
- Codex;
- ChatGPT/Workspace Agents where officially supported;
- Claude Code/Gemini later;
- project/repository bootstrap;
- skill/config projection;
- host session/episode identity.

A host MAY use a provider, but is not a provider.

### Canonical model descriptor
Must support unknown/partial capability knowledge without invention. Provider discovery is authoritative for identity/existence; static knowledge may enrich only when its provenance and staleness are explicit.

### Canonical Run / RunEvent
Provider events become Majordomus events once at the adapter boundary.

## Governance additions

Add a blocking project rule only if no existing rule already expresses this:

> Provider-specific semantics terminate at the adapter boundary, and every external interface is a projection of canonical provider/run capabilities.

Do NOT create a new rule merely to restate `interfaces-are-projections`.

If necessary, extend an existing rule instead, respecting rule versioning semantics.

Add/update ADR(s) for:
- provider vs host distinction;
- discovery truth and capability provenance;
- credentials/secrets;
- run/event normalization;
- external network operations not being hot-path/repository-open behavior.

## Schemas/front matter

Any new tracked `.ai/` directory must:
- be reachable from the manifest's explicit section model, or be placed under an existing registered section correctly;
- have its context README contract where repository rules require it;
- use an existing kind/schema when semantically correct;
- introduce a new kind only when it represents a genuinely new noun and survives the `no-new-nouns` discipline.

Do not add a `providers:` section to `.ai/manifest.yaml` unless repository semantics genuinely require repository-authored provider declarations. Runtime provider configuration may belong elsewhere.

## Validation

Add executable governance tests proving:
- no projection directly depends on OpenAI transport structs;
- provider adapters may depend on transport implementation;
- core run/model/provider types do not depend on OpenAI-specific types;
- new generated/reference outputs are derived and checked;
- secrets are structurally redacted.

Deliver governance first so later prompts fail loudly if they violate it.
