+++
title = "Editors and agents"
description = "what Zed and Google Antigravity read from the repository: the skills and tasks projected by `majordomus generate editors`, ownership and conflict refusal, what is left to the person (MCP, settings, permissions) and why"
weight = 53
[extra]
source = "docs/EDITORS.md"
+++

{% raw %}

What Zed and Google Antigravity read from this repository, where each file comes from, and
what is deliberately left to the person. Behaviour as implemented and tested; where this
document and the executable disagree, the document is wrong and changes in the same commit.
The decision is [ADR 140](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0140-editors-and-agents-read-projections-of-the-layer-and-cargo-e.md);
the code is `apps/majordomus-cli/src/editors/`.

<div class="overflow-x-auto" tabindex="0">

| source | projection | read by |
|---|---|---|
| `.ai/repo/skills/<id>/SKILL.md` | `.agents/skills/<id>/SKILL.md` | Zed and Antigravity |
| `justfile`, `.just/*.just` | `.zed/tasks.json` | Zed |
| `.ai/repo/policy.yaml` | `AGENTS.md` (`generate providers`) | both |

</div>


## Commands

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| `majordomus generate editors` | write the projections; remove a projection whose skill is gone |
| `majordomus generate --check` | exit 10 when any projection is stale, missing or orphaned |
| MCP `majordomus_editors`, `GET /api/v1/editors`, `majordomus://editors` | `editors.status`: every adapter's capabilities with evidence, every file's inputs, content address and state, every finding |

</div>


Nothing is registered anywhere. A new skill under `.ai/repo/skills/` or a new public recipe in
the justfile appears in the projection at the next `majordomus generate`, and `--check` fails
until it has.

## What is projected

- **Skills.** One `SKILL.md` per active skill with the `name` and `description` both products
  require, the generated-file banner after the front matter, and a link to the definition. The
  procedure is never copied: the agent is sent to the canonical file.
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

<div class="overflow-x-auto" tabindex="0">

| | state | instead |
|---|---|---|
| Zed MCP (`context_servers`) | requires user action | add `{"context_servers": {"majordomus": {"command": "<checkout>/bin/majordomus-mcp", "args": []}}}` to your own Zed settings |
| Antigravity workspace MCP | unknown | the web docs name `.agents/mcp_config.json`, the 2.22.0 guide does not; add the same server to `~/.gemini/config/mcp_config.json` |
| `.zed/settings.json` | unsupported | no policy here differs from Zed's defaults; theme, fonts, keys and agent settings are yours |
| Antigravity workflows | deprecated | they stop working on 2026-10-19; skills replace them |
| approval and execution modes | requires user action | never projected |

</div>


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

Each adapter module (`editors/zed.rs`, `editors/antigravity.rs`) names the release its claims
were verified against and the sources. When a product changes a format, change its module,
bump `editors::PROJECTION_VERSION` if the bytes change, and regenerate.
{% endraw %}
