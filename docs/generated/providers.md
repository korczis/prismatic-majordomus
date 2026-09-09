<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the provider declarations the distribution ships (share/providers.yaml), the templates beside them, and this repository's policy; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.1 -->
# Providers

Every provider the tool ships an adapter for, and what this repository does with each. The set is the templates under `share/providers/`; the title, the client configuration a provider reads and the scratch roots it creates checkouts under are `share/providers.yaml`; the bootstraps are the policy's `projections[]`; whether a client configuration is present and which hooks are wired are facts of this tree. `majordomus product providers`, the MCP tool `majordomus_providers`, `GET /api/v1/product/providers` and the site's provider cards answer from the same value. A document that names providers points here rather than listing them (ADR 0024).

| provider | title | bootstraps | client configuration | hooks | scratch roots |
|---|---|---|---|---|---|
| `agents` | Any tool that reads AGENTS.md | `AGENTS.md` (always loaded) | — | — | — |
| `bb` | bb | `.bb/AGENTS.md` | — | — | `${BB_DATA_DIR:-~/.bb}/plugins/environment-git-worktree/host-data/worktrees` |
| `claude-code` | Claude Code | `CLAUDE.md` | `.mcp.json` | `prompt-capture`, `session-lifecycle` | `<primary>/.claude/worktrees` |
| `codex` | Codex | — | `.codex/config.toml` | — | — |
| `gemini` | Gemini CLI | — | `.gemini/settings.json` | — | — |
| `generic` | A worker with no convention of its own | — | — | — | — |

6 provider(s).

## The tool's own scratch roots

A checkout under any of these, or under a provider's root above, is a session's scratch checkout to the worktree topology: reported, never migrated unasked, never cleaned up by the tool, refused by the commit guard with the remedy of continuing in the canonical worktree. A root the primary checkout itself lives under is skipped.

- `$TMPDIR`
- `/tmp`
- `/private/tmp`
- `/var/tmp`
- `/var/folders`
- `/private/var/folders`

A root is expanded before it is compared: `<primary>/` is the primary checkout, `~` the home directory, `${NAME:-default}` an environment variable with a default, `$NAME` one without — a root whose variable is unset is skipped.
