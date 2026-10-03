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


# Mission 06 — Ideas/intake pipeline and historical ChatGPT ingestion without transcript rot

Solve the user's actual problem: many partially developed ideas live in ChatGPT conversations and future ideas should be captured into Majordomus at creation time.

Do NOT make raw transcripts a new canonical knowledge store.

## First inspect existing nouns

Before inventing `Idea`, inspect:
- issue;
- milestone;
- prompt;
- claim;
- question;
- decision;
- knowledge;
- session;
- handover;
- any existing inbox/intake object.

If an existing noun cleanly models an uncommitted idea, extend it. If not, justify a new kind with an ADR/schema/use case.

## Semantic intake object

A semantic idea/intake record should be compact and curated:
- stable id;
- title;
- summary;
- state;
- source references;
- assertions;
- open questions;
- proposed consequences/changes;
- relations to issues/ADRs/rules/skills/knowledge;
- provenance;
- timestamps;
- optional confidence/triage metadata only if repository conventions support it.

Lifecycle should support capture → triage → exploration/validation → promotion/rejection.

Promotion must create or link canonical issue/ADR/rule/skill/etc rather than copying text blindly.

## Active capture

Expose a mutation capability usable through MCP so a ChatGPT/Codex/Claude session can say:
"capture this as an idea but do not make an issue yet."

Repository mutation deserves explicit policy and tests. Existing MCP docs currently call out repository mutation as intentionally absent, so this is an architectural decision. Do not smuggle writes into an existing read-only capability.

Requirements:
- scoped mutation;
- explicit source/provenance;
- no arbitrary path write;
- schema validation;
- git/worktree/task safety;
- conflict handling;
- idempotency/dedup key support;
- audit entry;
- user-visible confirmation object.

## Historical ChatGPT import

Implement import as a bounded adapter over a supported exported format or user-supplied export.

Pipeline:
1. parse export;
2. identify selected project/conversation scope where the export supports it;
3. create ephemeral transcript representation;
4. extract candidate ideas/prompts/decisions/questions with provenance;
5. deduplicate;
6. present dry-run;
7. import only canonical semantic objects;
8. discard raw transcript unless user explicitly requests local retention and policy allows it.

Never commit raw transcript archives to `.ai/repo/`.

Provide:
- `--dry-run`;
- deterministic import report;
- source hashes;
- restart/idempotency behavior;
- redaction of obvious secret-like material;
- tests with synthetic exports.

## Cockpit later

Prepare capability outputs so Cockpit can present:
- idea inbox;
- status;
- sources;
- links;
- promotion actions,
without embedding business rules in the UI.
