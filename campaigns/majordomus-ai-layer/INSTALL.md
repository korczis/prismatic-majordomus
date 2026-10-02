# Installation

This archive is a repository overlay for Majordomus.

## Add to repository root

Copy these paths into the repository root:

- `.ai/`
- `AGENTS.md`
- `CLAUDE.md`
- `GEMINI.md`
- `scripts/check-ai-layer.sh`

Merge `README_AI_SECTION.md` into the project's existing `README.md`. Do not replace an existing README blindly.

For a ChatGPT Project, paste `PROJECT_INSTRUCTIONS.txt` into Project Instructions.

## Verify

Run:

```sh
./scripts/check-ai-layer.sh
```

Then verify each provider in a fresh session:

- Codex: confirm `AGENTS.md` is active and that it reads the mandatory `.ai/` files.
- Claude Code: use its memory/context inspection to confirm `CLAUDE.md` and imports are loaded.
- Gemini CLI: use `/memory show` to confirm imported `.ai/` context.
- Antigravity / AGY: verify the root `AGENTS.md` is parsed on workspace startup.

## Maintenance

Never copy a shared rule into every provider bootstrap.

If the rule is provider-neutral, it belongs under `.ai/`.
