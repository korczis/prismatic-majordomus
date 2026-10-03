# Majordomus Agent Bootstrap

Canonical AI/LLM project instructions live in `.ai/`.

Before substantive planning, implementation, review, or repository mutation, read `.ai/00_README.md` and all mandatory files listed there.

At minimum, load:

- `.ai/10_CORE.md`
- `.ai/20_ARCHITECTURE.md`
- `.ai/30_DEVELOPMENT.md`
- `.ai/40_TESTING.md`
- `.ai/50_PRISMATIC_PORTING.md`
- `.ai/60_ROADMAP.md`

## Critical boundary

Prismatic may be used as inspiration or implementation reference, but Majordomus MUST NEVER directly depend on Prismatic. Useful functionality must be ported, adapted, or independently reimplemented inside Majordomus.

## Maintenance rule

`AGENTS.md` is an adapter, not the source of truth.

Do not duplicate shared rules here. Modify `.ai/` instead.

Provider-specific guidance may add mechanics but must not weaken or redefine `.ai/` rules.
