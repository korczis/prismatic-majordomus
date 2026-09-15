# Campaign briefs

Operator-authored prompt packs, kept verbatim as provenance. Each one is a brief that was
handed to an agent working in this repository: what was asked for, when, and in what order it
was to be done.

**These are history, not instructions.** Nothing here is loaded by the tool, validated as a
document of the layer, or published to the site, and no file in this directory decides anything
about the repository as it stands. Where a pack and the repository disagree, the repository is
right: the pack states what was wanted on its date, and what came of it is the tree, its rules
and its tests. Read one to learn why a subsystem exists, never to re-run it.

They are tracked because they were not. Each pack arrived as a download and was executed from
`~/Downloads` or from the repository's ignored `tmp/`, which means the brief behind a subsystem
survived only on one disk, in a directory git was told to ignore. `.ai/repo/scope.yaml` declares
`campaigns/**` so a worker may read them; `MANIFEST.sha256` records the bytes.

| pack | of | files | first heading | taken from |
|---|---|---|---|---|
| `majordomus-ai-layer` | 2026-09-04 | 16 | Majordomus Agent Bootstrap | `~/Downloads/majordomus-ai-layer` |
| `majordomus-ai-transform-pack` | 2026-09-04 | 32 | Majordomus `.ai/` Transformation Pack | `~/Downloads/majordomus-ai-transform-pack` |
| `majordomus-auto-control-plane-pack` | 2026-09-09 | 13 | Majordomus Automatic Control Plane Prompt Pack | `tmp/packs/majordomus-auto-control-plane-pack` |
| `majordomus-chatgpt-project-pack` | 2026-09-04 | 8 | Majordomus ChatGPT Project Pack | `~/Downloads/majordomus-chatgpt-project-pack` |
| `majordomus-chatgpt-project-provider-prompts` | 2026-09-09 | 14 | Majordomus ChatGPT Project Provider — Claude Code Prompt Pack | `tmp/packs/majordomus-chatgpt-project-provider-prompts` |
| `majordomus-cockpit-ide-prompt-pack-v1` | 2026-09-10 | 19 | Majordomus Cockpit IDE / Control Plane Prompt Pack | `tmp/prompt-packs/majordomus-cockpit-ide-prompt-pack-v1` |
| `majordomus-cockpit-landing-claude-code-prompt-pack` | 2026-09-11 | 14 | Majordomus Cockpit Landing Page — Claude Code Prompt Pack | `tmp/prompt-packs/majordomus-cockpit-landing-claude-code-prompt-pack` |
| `majordomus-cockpit-orchestration-prompt-pack` | 2026-09-10 | 14 | Common Contract — Majordomus Cockpit Runtime Orchestration | `tmp/packs/majordomus-cockpit-orchestration-prompt-pack` |
| `majordomus-convergence-prompt-pack-20260911` | 2026-09-11 | 26 | Majordomus Convergence Prompt Pack | `tmp/prompt-packs/majordomus-convergence-prompt-pack-20260911` |
| `majordomus-github-workgraph-prompt-pack` | 2026-09-09 | 17 | Majordomus GitHub / Work Graph Prompt Pack | `~/Downloads/majordomus-github-workgraph-prompt-pack` |
| `majordomus-openai-provider-control-plane-prompts` | 2026-09-07 | 14 | Majordomus OpenAI / Provider Control Plane Prompt Pack | `~/Downloads/majordomus-openai-provider-control-plane-prompts` |
| `majordomus-prismatic-import-pack` | 2026-09-09 | 12 | Majordomus ← Prismatic Platform: Skills & Doctrines Import Pack | `~/Downloads/majordomus-prismatic-import-pack` |
| `majordomus-provider-context-prompt-pack` | 2026-09-13 | 10 | Majordomus Provider + Session Continuity Prompt Pack | `tmp/prompt-packs/majordomus-provider-context-prompt-pack` |
| `majordomus-rks-fable-1m-prompts` | 2026-09-07 | 13 | Majordomus Repository Knowledge System (RKS) — Claude Code Fable 1M Integration Pack | `tmp/prompt-packs/majordomus-rks-fable-1m-prompts` |
| `majordomus-two-folder-consolidation-prompts` | 2026-09-06 | 16 | Shared execution contract | `tmp/packs/majordomus-two-folder-consolidation-prompts` |
| `majordomus-worktree-cleanup-pack` | 2026-09-09 | 16 | Majordomus Worktree Cleanup & Merge Campaign | `~/Downloads/majordomus-worktree-cleanup-pack` |
| `prismatic-majordomus-evening-project-v0.1` | 2026-09-03 | 43 | Prismatic Majordomus | `~/Downloads/prismatic-majordomus` |

## What came of them

The outcomes are measurable in this tree rather than claimed here. The `.ai/` layer, the work
graph under `.ai/repo/project/` with its GitHub projection, the skills and rules sections, the
worktree topology rule with `docs/WORKTREES.md`, and ADR 0032 on external workspaces each began
as one of these briefs. Several packs were also executed only in part, and a pack is not
evidence that what it asked for exists — the gates are.

## Adding one

Copy the pack directory in unmodified, regenerate `MANIFEST.sha256`, and add its row above:

```console
$ cp -R ~/Downloads/<pack> campaigns/<pack>
$ (cd campaigns && find . -type f ! -name MANIFEST.sha256 | LC_ALL=C sort | xargs shasum -a 256 | sed 's|\./||' > MANIFEST.sha256)
```

