# Prompt 03 — Import and Adapt Relevant Skills

Read prior prompt outputs and the migration matrix.

Implement the approved skill migrations.

## Rules

For every donor skill classified `IMPORT_AS_IS`, `ADAPT`, `REIMPLEMENT`, or `MERGE`:

1. inspect its complete source, not merely the headline Markdown;
2. trace dependencies;
3. understand activation/discovery semantics;
4. identify source-specific assumptions;
5. map it to the canonical Majordomus skill model;
6. preserve useful intent and rigor;
7. remove Prismatic-specific coupling unless genuinely reusable;
8. integrate with target schemas/registry;
9. write tests;
10. document it;
11. connect enforcement/diagnostics if the skill makes normative claims;
12. ensure applicable surfaces derive it automatically.

## Do not blindly copy

Especially reject or rewrite:

- absolute paths;
- source-specific app names;
- source-only commands;
- bespoke environment assumptions;
- obsolete provider names;
- private/local credentials;
- hardcoded inventories;
- references to infrastructure Majordomus does not have;
- duplicated guidance already canonical elsewhere.

## Skill quality bar

Each imported skill should make explicit, in the canonical target format:

- purpose;
- when applicable;
- inputs;
- outputs;
- preconditions;
- procedure/workflow;
- failure modes;
- validation;
- related doctrines/rules;
- related commands/capabilities;
- examples only where useful;
- provenance.

Where a skill contains executable behavior or tool integration, wire it through existing implementation layers rather than pretending Markdown is execution.

## Runtime and agent use

Inspect how Majordomus currently exposes skills to agents / LLM sessions / MCP / project initialization.

Imported skills should participate in that same mechanism.

Where applicable validate:

- discovery from a fresh session;
- canonical listing;
- machine-readable representation;
- explain/details view;
- applicability matching;
- target project installation/bootstrap;
- no duplicate registration.

## Extensibility test

Create a temporary representative skill fixture using the canonical format and prove it propagates automatically through the relevant registry and projections. Remove the fixture afterward unless repository test conventions retain fixtures.

## Legacy merge

If Majordomus already has a partial equivalent, merge into the stronger canonical implementation. Delete obsolete duplicate artifacts once no consumer depends on them.

Do not preserve two nearly identical skills merely to avoid deciding which one owns reality.
