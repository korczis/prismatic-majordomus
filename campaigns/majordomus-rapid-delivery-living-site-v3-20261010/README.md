# Majordomus Rapid Delivery & Living Landing Page V3

**Purpose:** Acceleration and cleanup of the existing V2 autonomous-delivery program, plus immediate public-facing site convergence. This is a prompt pack, not an assertion that any code was deployed.

## Based on observations as of 2026-10-10
- Fetched homepage advertised 0.19.0 while install docs advertised 0.19.1; repository history references 0.20.0. Treat this as a deployment/projection reconciliation task, not proof of a particular latest live release.
- Product feature documentation describes one file under `.ai/repo/features/` projected to CLI, API, OpenAPI, MCP, graph, matrix and site.
- Earlier V2 has 10 milestones and 62 issues; this pack MUST reconcile and reuse it, not blindly add duplicate issues.
- Website currently starts with an abstract supervisory claim; optimize comprehension and onboarding, preserve accurate safeguards and limitations.

## Contents
- `MASTER-EXECUTION-PROMPT.md` orchestration instruction
- `manifest.json` machine-readable milestone and issue plan
- `prompts/*.md` nine granular milestones with six issue work items each
- `TRACKING.md` human-readable milestone ledger and evidence contract
- `SITE-ACCEPTANCE.md` creative/technical brief + release delta acceptance

## Operating instructions
1. Place/unpack the pack in an accessible working directory.
2. In Claude Code inside the repo, provide: `Read and execute /absolute/path/MASTER-EXECUTION-PROMPT.md, autonomously via existing Majordomus mesh and canonical .ai lifecycle; begin M00.`
3. Do not pretend one Claude session is durable. Supervisor implementation must run independently of conversation context; preserve recovery checkpoints.
4. Enable and verify actual repository/environment permissions before expecting unattended merge/release/deploy; do not turn off security gates.
5. Canonical feature source and runtime release evidence win over editorial wishes.

## Sources to inspect in live repo
`.ai/repo/features/`, `.ai/repo/ci/`, release analyzer/version schema, site generator, homepage templates, docs generator, GitHub release workflows, deployment config, `prs drain`, mesh worker/supervisor, current PRs and latest master. Discover real paths before editing.
