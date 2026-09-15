## AI / LLM development context

Majordomus keeps shared AI-agent instructions in the repository-local [`.ai/`](.ai/) directory.

`.ai/` is the **canonical, provider-neutral source of truth** for architecture rules, development workflow, testing/guarantee discipline, roadmap constraints, and Prismatic porting policy.

Provider-specific files are deliberately thin bootstrap adapters:

| Tool | Bootstrap |
|---|---|
| OpenAI Codex | [`AGENTS.md`](AGENTS.md) |
| Claude Code | [`CLAUDE.md`](CLAUDE.md) |
| Gemini CLI | [`GEMINI.md`](GEMINI.md) |
| Google Antigravity / AGY | [`AGENTS.md`](AGENTS.md) |

The provider bootstrap is loaded using each tool's native project-instruction mechanism and then imports or directs the agent to the shared `.ai/` content.

**Do not duplicate project rules across provider files.** If a shared rule changes, change it once under `.ai/`.

Start with [`.ai/00_README.md`](.ai/00_README.md).

### Prismatic boundary

Prismatic may be used as prior art, inspiration, or implementation reference. Majordomus must never directly depend on Prismatic. Relevant functionality must be ported, adapted, or independently reimplemented inside Majordomus with Majordomus-native APIs, tests, documentation, and lifecycle.
