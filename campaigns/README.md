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
| `bro-net-majordomus-prompt-pack` | 2026-09-24 | 13 | Bro-Net × Majordomus Prompt Pack | `~/Downloads/bro-net-majordomus-prompt-pack.zip` |
| `majordomus-0.15.0-release-prompt-pack` | 2026-10-07 | 13 | Majordomus 0.15.0 — Same-Day Release Prompt Pack | `~/Downloads/majordomus-0.15.0-release-prompt-pack.zip` |
| `majordomus-ai-layer` | 2026-09-04 | 16 | Majordomus Agent Bootstrap | `~/Downloads/majordomus-ai-layer` |
| `majordomus-ai-transform-pack` | 2026-09-04 | 32 | Majordomus `.ai/` Transformation Pack | `~/Downloads/majordomus-ai-transform-pack` |
| `majordomus-auto-control-plane-pack` | 2026-09-09 | 13 | Majordomus Automatic Control Plane Prompt Pack | `tmp/packs/majordomus-auto-control-plane-pack` |
| `majordomus-autopilot-pack-20261010` | 2026-10-10 | 8 | Majordomus Autonomous Delivery Convergence · 2026-10-10 | `~/Downloads/majordomus-autopilot-pack-20261010.zip` |
| `majordomus-chatgpt-project-pack` | 2026-09-04 | 8 | Majordomus ChatGPT Project Pack | `~/Downloads/majordomus-chatgpt-project-pack` |
| `majordomus-chatgpt-project-provider-prompts` | 2026-09-09 | 14 | Majordomus ChatGPT Project Provider — Claude Code Prompt Pack | `tmp/packs/majordomus-chatgpt-project-provider-prompts` |
| `majordomus-cockpit-ide-prompt-pack-v1` | 2026-09-10 | 19 | Majordomus Cockpit IDE / Control Plane Prompt Pack | `tmp/prompt-packs/majordomus-cockpit-ide-prompt-pack-v1` |
| `majordomus-cockpit-landing-claude-code-prompt-pack` | 2026-09-11 | 14 | Majordomus Cockpit Landing Page — Claude Code Prompt Pack | `tmp/prompt-packs/majordomus-cockpit-landing-claude-code-prompt-pack` |
| `majordomus-cockpit-orchestration-prompt-pack` | 2026-09-10 | 14 | Common Contract — Majordomus Cockpit Runtime Orchestration | `tmp/packs/majordomus-cockpit-orchestration-prompt-pack` |
| `majordomus-convergence-pack-20261010` | 2026-10-10 | 8 | Majordomus: Verified Delivery Convergence | `~/Downloads/majordomus-convergence-pack-20261010.zip` |
| `majordomus-convergence-prompt-pack-20260911` | 2026-09-11 | 26 | Majordomus Convergence Prompt Pack | `tmp/prompt-packs/majordomus-convergence-prompt-pack-20260911` |
| `majordomus-convergence-prompt-pack-20261010` | 2026-10-10 | 11 | Majordomus Convergence / Delivery Acceleration Pack | `~/Downloads/majordomus-convergence-prompt-pack-20261010.zip` |
| `majordomus-delivery-acceleration-prompt-pack-20261011` | 2026-10-10 | 15 | MAJORDOMUS: DELIVERY CONVERGENCE CAMPAIGN | `~/Downloads/majordomus-delivery-acceleration-prompt-pack-20261011.zip` |
| `majordomus-derive-infer-enforce-prompt-pack` | 2026-10-11 | 8 | Prompt pack: Majordomus derive/infer/verify/enforce | `~/Downloads/majordomus-derive-infer-enforce-prompt-pack.zip` |
| `majordomus-github-workgraph-prompt-pack` | 2026-09-09 | 17 | Majordomus GitHub / Work Graph Prompt Pack | `~/Downloads/majordomus-github-workgraph-prompt-pack` |
| `majordomus-idd-completion-prompt-pack` | 2026-09-16 | 15 | Majordomus IDD Completion Prompt Pack | `~/Downloads/majordomus-idd-completion-prompt-pack.zip` |
| `majordomus-intent-closure-pack` | 2026-10-07 | 12 | Majordomus Intent Closure Pack | `~/Downloads/majordomus-intent-closure-pack.zip` |
| `majordomus-intent-outcome-prompt-pack` | 2026-10-08 | 13 | Majordomus: Intent → Verified Outcome | `~/Downloads/majordomus-intent-outcome-prompt-pack.zip` |
| `majordomus-intent-tight-integration-pack` | 2026-10-07 | 15 | Majordomus Intent Tight Integration Pack | `~/Downloads/majordomus-intent-tight-integration-pack.zip` |
| `majordomus-landing-atomic-pack` | 2026-10-10 | 21 | Majordomus Public Surface: Atomic Autonomous Execution Pack | `~/Downloads/majordomus-landing-atomic-pack.zip` (its `MAJORDOMUS_ATOMIC_PROMPT_PACK.md` also arrived alone, byte for byte) |
| `majordomus-mesh-pack` | 2026-10-10 | 23 | Majordomus Mesh + Prompt-Pack Lifecycle — Opus/Fable Prompt Pack | `~/Downloads/majordomus-opus-fable-full-prompt-pack-20261010.zip` |
| `majordomus-openai-provider-control-plane-prompts` | 2026-09-07 | 14 | Majordomus OpenAI / Provider Control Plane Prompt Pack | `~/Downloads/majordomus-openai-provider-control-plane-prompts` |
| `majordomus-opus55-recovery-pack` | 2026-10-08 | 19 | Prismatic Majordomus: Opus 5.5 Recovery & Evidence-First Prompt Pack | `~/Downloads/majordomus-opus55-recovery-pack.zip` |
| `majordomus-pr-throughput-pack-20261010` | 2026-10-10 | 18 | Majordomus PR Throughput: closed-loop integration and release | `~/Downloads/majordomus-pr-throughput-pack-20261010.zip` |
| `majordomus-prismatic-import-pack` | 2026-09-09 | 12 | Majordomus ← Prismatic Platform: Skills & Doctrines Import Pack | `~/Downloads/majordomus-prismatic-import-pack` |
| `majordomus-provider-context-prompt-pack` | 2026-09-13 | 10 | Majordomus Provider + Session Continuity Prompt Pack | `tmp/prompt-packs/majordomus-provider-context-prompt-pack` |
| `majordomus-rapid-delivery-living-site-v3-20261010` | 2026-10-10 | 14 | Majordomus Rapid Delivery & Living Landing Page V3 | `tmp/owner-packs/majordomus-rapid-delivery-living-site-v3-20261010` |
| `majordomus-release-15min-pack` | 2026-10-10 | 9 | Majordomus Release in 15 Minutes · 2026-10-10 | `tmp/owner-packs/majordomus-release-15min-pack` |
| `majordomus-rks-fable-1m-prompts` | 2026-09-07 | 13 | Majordomus Repository Knowledge System (RKS) — Claude Code Fable 1M Integration Pack | `tmp/prompt-packs/majordomus-rks-fable-1m-prompts` |
| `majordomus-runtime-mcp-mesh-opus-fable-pack` | 2026-10-09 | 14 | Majordomus Runtime / MCP / Mesh Convergence | `~/Downloads/majordomus-runtime-mcp-mesh-opus-fable-pack.zip` (also unpacked there as `majordomus-runtime-prompt-pack`, the same bytes) |
| `majordomus-two-folder-consolidation-prompts` | 2026-09-06 | 16 | Shared execution contract | `tmp/packs/majordomus-two-folder-consolidation-prompts` |
| `majordomus-ui-implementation-prompt-pack` | 2026-10-09 | 14 | Majordomus UI — executable implementation prompt pack | `~/Downloads/majordomus-ui-implementation-prompt-pack.zip` |
| `majordomus-ui-implementation-prompt-pack-v1.1` | 2026-10-09 | 15 | Majordomus UI — executable implementation prompt pack | `~/Downloads/majordomus-ui-implementation-prompt-pack-v1.1.zip` |
| `majordomus-ui-ux-dx-prompt-pack` | 2026-10-05 | 15 | Majordomus UI/UX/DX Prompt Pack | `~/Downloads/majordomus-ui-ux-dx-prompt-pack.zip` |
| `majordomus-worktree-cleanup-pack` | 2026-09-09 | 16 | Majordomus Worktree Cleanup & Merge Campaign | `~/Downloads/majordomus-worktree-cleanup-pack` |
| `prismatic-majordomus-evening-project-v0.1` | 2026-09-03 | 43 | Prismatic Majordomus | `~/Downloads/prismatic-majordomus` |

## What came of them

The outcomes are measurable in this tree rather than claimed here. The `.ai/` layer, the work
graph under `.ai/repo/project/` with its GitHub projection, the skills and rules sections, the
worktree topology rule with `docs/WORKTREES.md`, and ADR 0032 on external workspaces each began
as one of these briefs. Several packs were also executed only in part, and a pack is not
evidence that what it asked for exists — the gates are.

## Where each pack is planned

A pack is a brief, and the plan is the repository's: each pack below became milestones and
issues under `.ai/repo/project/`, and each such record names its pack by the path it is kept at
here. Both directions are the point. A record citing a pack that is not kept is a citation
nobody can open, and a pack nothing cites is a brief nobody acted on, so the table names, for
every pack added on 2026-10-11, where its plan was written, and the records cite back by path.
Where a plan is not yet on master, the branch carries it.

| pack | planned as | by |
|---|---|---|
| `majordomus-intent-closure-pack`, `majordomus-intent-tight-integration-pack` | I2200-I2214 (intent-bound-work, an-outcome-becomes-a-reviewed-plan, the-intent-loop-is-proved-end-to-end) | p11, `chore/the-owner-packs-are-planned` |
| `majordomus-idd-completion-prompt-pack`, `majordomus-intent-outcome-prompt-pack` | I2215-I2225 (an-unmet-criterion-is-drafted-into-work, command-and-deployment-criteria-are-judged) | p11, `chore/the-owner-packs-are-planned` |
| `majordomus-ui-implementation-prompt-pack-v1.1`, `majordomus-ui-ux-dx-prompt-pack` | I2226-I2234 (a-fact-is-written-once, surfaces-never-disagree, cockpit-control-center); v1.0 is superseded by v1.1 | p11, `chore/the-owner-packs-are-planned` |
| `majordomus-opus55-recovery-pack`, `bro-net-majordomus-prompt-pack`, `majordomus-0.15.0-release-prompt-pack` | the remaining owner-pack issues up to I2281 (recovery, a-browser-is-a-read-only-peer, a-release-is-followed-to-its-users); 0.15.0 is mostly obsolete | p11, `chore/the-owner-packs-are-planned` |
| `majordomus-runtime-mcp-mesh-opus-fable-pack` | I2126-I2165 and I2285-I2300 (runtime-lifecycle, runtime-trust, mcp-transports-are-reliable, mesh-journal-converges) | 1b, `chore/the-runtime-pack-is-planned` |
| `majordomus-autopilot-pack-20261010`, `majordomus-convergence-pack-20261010`, `majordomus-convergence-prompt-pack-20261010` | I2310-I2389 (delivery-baseline-is-measured through the-delivery-loop-is-proved) | b2, `chore/the-autopilot-and-convergence-packs-are-planned` |
| `majordomus-delivery-acceleration-prompt-pack-20261011` | CI Delivery V4: I2396, I2397, I2416 (test-evidence) | ce, with I2416 by this session |
| `majordomus-landing-atomic-pack`, `majordomus-derive-infer-enforce-prompt-pack` | I2420-I2423 (a-visitor-finds-any-public-object, a-claim-page-shows-its-whole-proof); every other atom maps to an existing record, listed in those milestones | ce, `chore/the-atomic-and-derive-packs-are-planned` |
| `majordomus-pr-throughput-pack-20261010`, `majordomus-release-15min-pack`, `majordomus-rapid-delivery-living-site-v3-20261010` | executed by the integration executors (`majordomus prs`, the landing train) and the release chain; no separate plan records | cd, 3d |

## Adding one

Copy the pack directory in unmodified, regenerate `MANIFEST.sha256`, and add its row above.
Only the top-level manifest is left out of its own list; a pack's own `MANIFEST.sha256` is a
byte of the brief and is recorded like any other. Hash with `sha256sum`, or with
`openssl dgst -sha256 -r` where it is absent, never `shasum`, which is a Perl script on macOS:

```console
$ cp -R ~/Downloads/<pack> campaigns/<pack>
$ (cd campaigns && find . -type f ! -path ./MANIFEST.sha256 -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sed 's|^\([0-9a-f]*\) [ *]\./|\1  |' > MANIFEST.sha256)
```

