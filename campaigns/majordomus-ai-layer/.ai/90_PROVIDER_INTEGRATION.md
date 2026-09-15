# AI Provider Integration

`.ai/` is canonical. Provider entrypoints are adapters only.

## Rules

1. Shared project semantics belong in `.ai/`.
2. Provider-specific root files must stay short.
3. Do not maintain copied variants of the same rule.
4. Provider files may contain only:
   - native import/include directives,
   - bootstrap/read instructions,
   - provider-specific operational mechanics.
5. A provider adapter must not weaken `.ai/` rules.
6. New AI tooling should integrate by loading the existing `.ai/` content, not by creating another canonical ruleset.

## Native entrypoints

| Tool | Native project entrypoint | Strategy |
|---|---|---|
| OpenAI Codex | `AGENTS.md` | Automatically loaded; instructs Codex to load canonical `.ai/` files |
| Claude Code | `CLAUDE.md` | Automatically loaded; imports canonical `.ai/` files |
| Gemini CLI | `GEMINI.md` | Automatically loaded; imports canonical `.ai/` files |
| Google Antigravity / AGY | `AGENTS.md` and workspace rules | Root `AGENTS.md` bootstraps the canonical `.ai/` layer |

## Adding another provider

Do not edit all shared rules.

Instead:

1. identify the provider's native auto-loaded project instruction file,
2. create the thinnest possible bootstrap,
3. have it import or explicitly load `.ai/`,
4. document only provider-specific mechanics in that bootstrap,
5. verify in a fresh session that the shared instructions are actually visible.

If a provider supports no file imports, its bootstrap must explicitly instruct the agent to read the mandatory `.ai/` files before substantive work.
