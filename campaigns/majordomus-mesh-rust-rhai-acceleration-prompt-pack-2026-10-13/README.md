# Majordomus Mesh Acceleration Prompt Pack · 2026-10-13

Mission: consolidate the **existing** Majordomus architecture around canonical Rust capabilities + bounded Rhai composition + externally installable plugins, eliminate domain shell logic, improve graph intelligence, clear verified CI/PR debt, and ship short, safe releases through the existing train.

## How to run

1. Read `START-HERE.md` and `contracts/ACCEPTANCE.md`.
2. Paste the entirety of `prompts/00-MESH-COMMANDER.md` into your **primary** Claude Code instance in the repo.
3. Paste specialized `prompts/01-...` to `prompts/11-...` into other instances **only if** they are available and the board shows scopes are free. Every worker joins the canonical Majordomus task/mesh state, not a separate plan. Commander may distribute them itself if existing capabilities allow.
4. Keep no more than a few concurrent writable scopes until measured throughput warrants more.
5. Follow `runbooks/RELEASE-WAVES.md`, and let actual repo governance outrank these instructions.

Prompts are AI-to-AI execution mandates, deliberately detailed. File paths and historical findings are leads, **not** verified assertions about current HEAD. Do not interpret these prompt files as executable shell scripts. Nothing in this ZIP modifies GitHub or deploys itself.

## Contents

- `prompts/00` Commander and orchestration
- `prompts/01` CI/evidence
- `prompts/02` PR/release integration
- `prompts/03` Rust capability kernel
- `prompts/04` Rhai engine
- `prompts/05` external plugins
- `prompts/06` shell/just migration
- `prompts/07` graph UX
- `prompts/08` derived surfaces/docs
- `prompts/09` security/adversarial
- `prompts/10` benchmark economics
- `prompts/11` release verification

## Sources and historical context

Grounded in the repo README, AGENTS.md, docs/SITE_CLAIMS.md and `.github/workflows/github.yml` reviewed on 2026-10-13 via connected GitHub, plus recent commits including `e506c041a44c` (2026-10-13 09:52Z), and the user's requirements in this session. A supplied 2026-09-11 repository archive is *historical only*. All commands/files must be discovered again at runtime. The observed 31 open PRs, shell/Rust GitHub-sync drift discrepancy (318 vs 26 in workflow comment), and PR #1060 cadence work are audit leads not guaranteed live defects.
