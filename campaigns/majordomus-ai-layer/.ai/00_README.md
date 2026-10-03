# Majordomus AI Layer

This directory is the canonical, provider-neutral instruction and context layer for AI/LLM coding tools working on Majordomus.

## Contract

`.ai/` is the source of truth for shared AI-facing project knowledge.

Provider-specific bootstrap files such as `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` MUST stay thin. They may describe provider mechanics, but MUST NOT fork or duplicate Majordomus rules.

When a shared rule changes, change it here once.

## Mandatory read order

Before substantive planning, implementation, review, or repository mutation, load:

1. `.ai/10_CORE.md`
2. `.ai/20_ARCHITECTURE.md`
3. `.ai/30_DEVELOPMENT.md`
4. `.ai/40_TESTING.md`
5. `.ai/50_PRISMATIC_PORTING.md`
6. `.ai/60_ROADMAP.md`

Use `.ai/90_PROVIDER_INTEGRATION.md` when changing AI-tool integration itself.

## Precedence

Use this precedence when instructions conflict:

1. explicit current user/operator instruction,
2. safety/security constraints,
3. repository-local scoped instructions for the file being changed,
4. `.ai/` shared Majordomus rules,
5. provider-specific convenience guidance.

Provider adapters MUST NOT weaken architectural or safety rules defined in `.ai/`.

## Design goals

The layer must be:

- LLM-provider independent,
- model independent,
- version controlled,
- readable by humans,
- usable from CLI and IDE agents,
- explicit about invariants and evidence,
- small enough to remain useful as context,
- free of provider-specific business logic.

Do not turn `.ai/` into a dumping ground for chat transcripts or generated scratch notes.
