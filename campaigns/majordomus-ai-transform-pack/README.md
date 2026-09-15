# Majordomus `.ai/` Transformation Pack

This directory is an operator-approved transformation brief for the current
`prismatic-majordomus` repository.

It is intended to be unpacked into:

```text
<repo>/tmp/transform/
```

and then handed to Claude Code while Claude operates on the repository root.

## Start here

Claude Code MUST read, in order:

1. `CLAUDE.md`
2. `00_EXECUTIVE_BRIEF.md`
3. `01_SOURCE_SNAPSHOT.md`
4. `02_DECISIONS_AND_INVARIANTS.md`
5. `03_TARGET_ARCHITECTURE.md`
6. `04_MIGRATION_MAP.yaml`
7. `05_AI_PROTOCOL.md`
8. `06_RULES_FORMAT.md`
9. `07_STATE_MODEL.md`
10. `08_BOOTSTRAP_AND_PROVIDER_MODEL.md`
11. `09_MIGRATION_DAG.md`
12. `10_ACCEPTANCE_CRITERIA.md`
13. `11_TEST_MATRIX.md`
14. `12_COMPATIBILITY_ROLLBACK.md`
15. `13_OPEN_DECISIONS.md`
16. `PROMPT_FOR_CLAUDE_CODE.md`

`PROMPT_FOR_CLAUDE_CODE.md` is the autonomous execution instruction. The other
files are normative supporting material.

## Authority

This pack records explicit operator decisions made after a forensic review of
the repository snapshot. Where the current repository's generated `AGENTS.md`,
`CLAUDE.md`, `.majordomus/` layout, documentation, or tests contradict the
target structure described here, treat the current files as **migration
subjects and evidence of existing behavior**, not as authority over the new
layout.

Do not weaken safety, behavioral guarantees, failure transparency, evidence
requirements, provider neutrality, or the clean-room boundary to Prismatic.

## Non-goal

This is not permission to redesign Majordomus into a general agent framework.
The transformation is primarily an ownership, context, state, and rule-model
normalization. Preserve observable behavior unless this pack explicitly changes
the contract.
