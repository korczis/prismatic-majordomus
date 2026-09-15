# Operator Instructions — Claude Code / Fable / Opus

Use a high-context model and run the numbered prompts sequentially in the SAME repository lineage, preferably one canonical worktree/task per coherent phase according to Majordomus governance.

## At the beginning of each prompt

Tell the agent:

```text
Continue the Cockpit IDE prompt pack. Read 00-README-FIRST.md and all completed phase reports.
Do not trust previous claims without source/test/runtime evidence.
Run the repository governance preflight before mutation.
Reuse existing canonical abstractions; do not build parallel registries.
Commit/push checkpoints according to repository rules.
```

Then paste the phase prompt.

## Between prompts

Require a phase report containing:
- evidence;
- changed files;
- tests;
- generated artifacts;
- docs;
- current branch/worktree;
- commit SHA;
- pushed state;
- open failures;
- assumptions invalidated.

Feed that report into the next prompt or store it using the repository's canonical session/handover mechanism.

## Do not let the agent do these things

- hand-edit generated `AGENTS.md` or `CLAUDE.md`;
- hand-edit generated OpenAPI;
- create Cockpit-only feature inventories;
- put repository semantics into JavaScript;
- introduce endpoint aliases with duplicate handlers;
- skip full tests because focused tests passed;
- call generated docs "updated" without drift check;
- call GH Pages "deployed" without workflow/runtime verification;
- leave unpushed commits;
- create parallel session/context storage;
- create a second server for Cockpit;
- shell back into Majordomus when direct capability execution exists.

## Recovery

If a phase discovers architectural contradiction, stop feature expansion, fix the canonical layer, update docs/ADR/rules/tests, then resume.

The pack is intentionally repetitive about proof because AI agents, like humans, are extremely capable of confusing "I wrote code" with "the system now works."
