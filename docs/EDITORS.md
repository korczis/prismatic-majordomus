# Editors and agents

What Zed, Google Antigravity and Claude Code read from this repository, where each file comes from, and
what is deliberately left to the person. Behaviour as implemented and tested; where this
document and the executable disagree, the document is wrong and changes in the same commit.
The decision is [ADR 140](../.ai/repo/adrs/0140-editors-and-agents-read-projections-of-the-layer-and-cargo-e.md);
the code is `apps/majordomus-cli/src/editors/`.

| source | projection | read by |
|---|---|---|
| `.ai/repo/skills/<id>/SKILL.md` | `.agents/skills/<id>/SKILL.md` | Zed and Antigravity |
| `.ai/repo/skills/<id>/SKILL.md` | `.claude/skills/<id>/SKILL.md` | Claude Code |
| a capability that declares `.skill(alias, argument)` | `.claude/skills/<alias>/SKILL.md` | Claude Code |
| every module of the registry with a public MCP tool | `.claude/skills/mj-<module>/SKILL.md` | Claude Code |
| `justfile`, `.just/*.just` | `.zed/tasks.json` | Zed |
| `.ai/repo/policy.yaml` | `AGENTS.md` (`generate providers`) | both |

## Commands

| | |
|---|---|
| `majordomus generate editors` | write the projections; remove a projection whose skill is gone |
| `majordomus generate --check` | exit 10 when any projection is stale, missing or orphaned |
| MCP `majordomus_editors`, `GET /api/v1/editors`, `majordomus://editors` | `editors.status`: every adapter's capabilities with evidence, every file's inputs, content address and state, every finding |

Nothing is registered anywhere. A new skill under `.ai/repo/skills/`, a new public recipe in
the justfile, or a capability that declares a skill appears in the projection at the next
`majordomus generate`, and `--check` fails until it has.

## What is projected

- **Skills.** One `SKILL.md` per active skill with the `name` and `description` both products
  require, the generated-file banner after the front matter, and a link to the definition. The
  procedure is never copied: the agent is sent to the canonical file. It is written twice,
  because the products look in two places: under `.agents/skills/` for Zed and Antigravity and
  under `.claude/skills/` for Claude Code, which reads no other directory, so that `/<id>`
  there reaches the layer's definition and not a command of the same name a person keeps in
  their own settings. The two files differ in the directory they name and in nothing else. A
  skill whose name a capability's command already has is written once, as the command, and
  the pair is a finding.
- **Commands.** One `SKILL.md` per capability whose declaration says `.skill(alias, argument)`,
  so that `/<alias> <text>` in Claude Code calls that capability once with the text in the
  named input field, verbatim. The file names the capability, its MCP tool and its command
  line, and nothing about what it does beyond the declaration's own title: the capability is
  defined once. A capability that changes something carries `disable-model-invocation: true`,
  so only a person invokes it, and the file says not to retry a refused or unknown outcome by
  another way. The registry refuses an alias that is malformed, reserved by an agent product
  or taken, and an argument that is not a string field of the input or leaves another field
  required. Today: `/intent`, which is `intents.propose`. Every other capability is reached by
  its MCP tool, and is listed in its domain's skill.
- **Domains.** One `SKILL.md` per module of the capability registry that offers a public
  capability with an MCP tool ([ADR 165](../.ai/repo/adrs/0165-a-skill-is-derived-for-every-domain-of-the-registry.md)),
  named `mj-<module>` with underscores as hyphens. Its description is the module and its
  capabilities' own titles, cut at a word to 300 characters; its body is a table of those
  capabilities with their MCP tool, command line and effect. The model may load it on its own,
  because loading it changes nothing; a capability marked *changes* runs only when the person
  asked for that change. A domain whose name a command or a workflow skill has is not written.
  The site's skills section (`/skills/`) and the UI's Skills screen (`/ui/skills`) list every
  Claude Code skill from the same projection; the rule is `project.every-domain-is-a-skill`.
- **Tasks.** Every public recipe of the tracked justfile that has no `[confirm]` and needs no
  argument, as `just <recipe>` from the project root, tagged with its group, in the order the
  justfile declares its groups. The local command-graph bridge the justfile also imports is
  not read, so the file is the same on every machine. Where the justfile exists and `just` cannot
  describe it, `generate` refuses with exit 12 rather than write a task list that depends on
  the machine; every CI job that builds the crate installs `just` through
  `.github/actions/setup-rust`.
- **Instructions.** Nothing new: both products read `AGENTS.md`, which `generate providers`
  derives from the policy.

## What is not, and what to do instead

| | state | instead |
|---|---|---|
| Zed MCP (`context_servers`) | requires user action | add `{"context_servers": {"majordomus": {"command": "<checkout>/bin/majordomus-mcp", "args": []}}}` to your own Zed settings |
| Antigravity workspace MCP | unknown | the web docs name `.agents/mcp_config.json`, the 2.22.0 guide does not; add the same server to `~/.gemini/config/mcp_config.json` |
| `.zed/settings.json` | unsupported | no policy here differs from Zed's defaults; theme, fonts, keys and agent settings are yours |
| Antigravity workflows | deprecated | they stop working on 2026-10-19; skills replace them |
| approval and execution modes | requires user action | never projected |

## Ownership

A projected file carries the generated-file banner. `majordomus generate editors` refuses to
overwrite a file at a projected path that does not — or one reached through a symbolic link —
and names each; move it aside or fold it into its canonical source. A banner-carrying skill
projection whose skill is gone is removed by `generate` and reported by `--check`. Writes go
through a temporary file and a rename, and a file whose bytes are already right is left alone.

`editors.status` also reports what an editor would do with files this generator does not own:
an instruction file Zed would read before `AGENTS.md`, a `.agents/workflows/` directory, a
`.agents/skills.json` (which would load the canonical files, without the `name` Antigravity's
guide requires, beside the projection), and an `.agents/rules/*.md` without a valid `trigger`,
which Antigravity discards.

## Updating an adapter

Each adapter module (`editors/zed.rs`, `editors/antigravity.rs`, `editors/claude.rs`) names the release its claims
were verified against and the sources. When a product changes a format, change its module,
bump `editors::PROJECTION_VERSION` if the bytes change, and regenerate.
