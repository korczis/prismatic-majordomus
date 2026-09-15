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


# Mission 02 — Provider kernel, configuration, credentials and model discovery

Implement the provider-neutral kernel before OpenAI.

## Canonical types

Design typed types roughly equivalent to, adapted to live repository conventions:

- `ProviderId`
- `ProviderDescriptor`
- `ProviderKind`
- `ProviderConfiguration`
- `CredentialRequirement`
- `CredentialState` (NEVER raw credential in serializable public output)
- `ProviderCapability`
- `CapabilityEvidence` / provenance
- `ModelId`
- `ModelDescriptor`
- `ModelCapabilitySet`
- `ProviderHealth`
- `ProviderAdapter` trait
- `ProviderRegistry`

Do not over-generalize. Every abstraction must have an immediate OpenAI use and plausible second-provider use.

## Configuration

Support a layered resolver compatible with repository conventions:
- explicit command input;
- repository/user Majordomus configuration where such a concept already exists;
- environment variables;
- safe defaults.

Do not make `.ai/repo/` carry machine-local secrets.

Separate:
- provider identity;
- endpoint/base URL;
- credential reference/slot;
- runtime status;
- discovered models.

## Credentials

Create a credential resolver abstraction with at least environment-backed resolution for OpenAI. It must:
- expose configured/missing/error state without values;
- redact `Debug`/Display/serde output;
- have leak-prevention tests;
- never enter generated files;
- never be returned by MCP/HTTP/Cockpit.

Future keychain/external-command support should be possible without encoding them now unless the live repo already has those facilities.

## Model discovery

Implement provider-driven discovery interface.

Requirements:
- no hard-coded current OpenAI model inventory in core;
- discovery results have timestamp/source/provenance;
- cache only through existing cache discipline;
- cache is invisible and rebuildable;
- offline/no-credential states are explicit;
- stale cache cannot masquerade as a live provider answer;
- deterministic normalized ordering;
- test fixtures never require real credentials.

Model capabilities may be:
- provider-declared;
- protocol-inferred;
- locally enriched;
- unknown.

Represent that provenance. Unknown means unknown, not false or true.

## Selection foundation

Implement a provider-neutral matcher sufficient for:
- require capabilities;
- optionally prefer capabilities/traits;
- constrain provider;
- constrain model id;
- deterministic result ordering.

Do not build a sprawling routing DSL yet.

## Canonical capabilities

Expose provider/model inspection through existing `capability!` architecture, not separate surface code.

Likely capability semantics:
- `providers.list`
- `providers.describe`
- `providers.health`
- `models.list`
- `models.describe`
- `models.select`

Adapt names to the repository's naming conventions after inspecting them.

Every capability must have typed schemas and benchmark/use-case evidence where required.
