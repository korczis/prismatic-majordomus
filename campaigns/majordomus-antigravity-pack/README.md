# Majordomus × Google Antigravity: Editor + Agent Intelligence Prompt Pack

**Date:** 2026-10-11. **Target:** Claude Code Opus/Fable implementing an Antigravity adapter in the actual `prismatic-majordomus` repository.

## Run

Open the repository in Claude Code. Read `00-MASTER-PROMPT.md` and execute phases `01` to `07` sequentially. Treat `08` as the conformance contract. Use repository-native commands and record evidence. This is a specification, **not** confirmation code has been implemented.

## Why Antigravity is not merely another Zed

It combines IDE configuration with project agent rules, discoverable skills, MCP, projects/worktrees, agent control and permission boundaries. The generic Editor Intelligence subsystem should be extended with agent capabilities rather than cloning .zed logic.

## Critical version warning

Current official pages disagree about legacy workflow retirement: one says **2026-10-19**, another **2026-11-01**. Both direct developers to Agent Skills. Do not generate `.agents/workflows/`; use `.agents/skills/<name>/SKILL.md`, and revalidate documentation during implementation.

## Official documentation consulted

- https://antigravity.google/docs/rules/
- https://antigravity.google/docs/skills
- https://antigravity.google/docs/mcp
- https://antigravity.google/docs/projects/
- https://antigravity.google/docs/settings
- https://antigravity.google/docs/agent-settings
- https://antigravity.google/docs/ide/workflows/
- https://antigravity.google/docs/migration/workflows-to-skills/

## Generated outputs to aim for

- Canonical typed models and adapter contract with safety rules.
- Native `.agents/rules/*.md`, `.agents/skills/*/SKILL.md`, `.agents/mcp_config.json` where supported and safe, all derived and ownership-aware.
- CLI render/diff/check/sync/explain/doctor, shared API/MCP/UI projection, tests, enforcement and documentation.
- Measured dogfooding and verified release when approved quality gates pass.

## Files

00 master mission; 01 audit; 02 core; 03 adapter; 04 MCP and intent loop; 05 safe sync; 06 enforcement/tests; 07 dogfood/release; 08 future-adapter interface.
