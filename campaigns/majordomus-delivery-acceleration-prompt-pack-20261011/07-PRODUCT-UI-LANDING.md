# PHASE F: USER-FACING PROJECT UI AND HONEST LANDING PAGE

Role: UI/UX/DX engineer. This is a projection improvement, not a frontend rewrite.


## NON-NEGOTIABLE EXECUTION CONTRACT
- Work against the ACTUAL current repository HEAD. Read AGENTS.md, `.ai/**`, governance/doctrines, schemas, ADRs, plan/issue model, commands/capability registry, CI workflows, docs/CLAIMS.yaml, documentation generation, site/cockpit rules, and tests BEFORE proposing changes. Discover paths; do not invent them. The September 2026 bundled archive is historic context, NOT current truth.
- The source of truth is the existing canonical typed repository model and Git. Inferred/derived means no shadow YAML registry, hardcoded frontend inventories, manually synchronized docs or generated facts. Respect existing local shell/Rust boundary; do not reflexively rewrite.
- Establish the state of feature by repo evidence: existing, partial, missing, blocked, obsolete. Explicitly separate implemented, merged, released, deployed, and verified-in-production. No claims of completion by intention or PR description alone.
- Prefer the smallest safe change that corrects a systemic bottleneck. Reuse existing commands, gates, logging, event formats, diagnostics, ownership and effects model. New abstractions need at least two proven consumers or a compelling correctness reason.
- Avoid unsafe auto-merge, branch deletion, release, remote fleet actions, weakened security, ignored exit codes, bypassed branch protection, or suppressed CI. Use established permissions, policy and authorization; never leak credentials. Automated changes only when authorized by existing policy. If blocked, record blocker and continue independent work.
- Repository work must be tracked by intent -> criteria -> milestone -> issue/task -> evidence where supported. Create/fix missing bindings through existing machinery rather than fictional names. Granular PRs, preferably single-purpose; separate generated artifacts where feasible.
- Enforce via actual tests/CI, not merely via a Markdown doctrine. For each new invariant create negative/mutation proof that would fail before the patch; maintain portable adopter behavior. Determinism across machines, worktrees, process schedules and retries is mandatory.
- Do not claim an unknown measurement. Report reproducible commands, revision IDs, expected vs observed results, limitations and exact remaining work. Fail closed on uncertainty for completion, releases and remote writes, without breaking read-only diagnostics.
- Progress autonomously through justified tasks, but stop any destructive/high-impact action outside existing authorization. If a real external approval is required, state the exact blocker and proceed on safe independent tasks.
- Keep PUBLIC docs and UI claims honest and evidence-backed. Changes to contracts require schema/version compatibility review and migrations. Build, test, regenerate, document, review, integrate, release and verify deployment only where permissions and real evidence permit.

## Initial exploration
Inspect existing site/cockpit UI rules, Flowbite recommendations present in repo, current templates, Alpine.js interactions, JS bundling, REST/OpenAPI, graph data, claims status, browser test infrastructure and mobile defects. Treat built-in design system as authoritative; no styling parallel universe.

## UX information architecture
Entry path: Choose/inspect project -> understand active intents -> milestone/task -> agent/peer activity -> evidence & blockers -> PR/release -> deployed outcome. Progressive disclosure: Understand / Operate / Inspect. Primary screen answers 'what changed, what is blocked, what requires me, what can I do now?' from canonical capabilities.

## Implementation
- Prefer existing UI stack; use Flowbite/Tailwind/Alpine only where already supported or justified by existing rules. Cytoscape for inspectable dependency graphs; ECharts for real sampled metrics; avoid p5.js unless an actual user requirement needs it.
- Every count, status, label, link target, action capability and provenance derives from real backend model; no handwritten simulated mesh or fictional performance charts. Document unavailable states explicitly.
- Navigation and deep links support project, intent, task, PR, peer, release and trace; preserve mobile URL/history and keyboard accessibility. User actions are authorized, idempotent and visibly report server confirmation/errors.
- First-run state and zero-config guidance: installation, repository scan, first task, what is guaranteed/planned. Explain missing permissions without dead buttons.
- Responsive browser scenarios: 320/375/390/768/1024/1440+ widths, working drawer/focus trap, touch targets, keyboard tab order, contrast, reduced motion, loading/empty/error/offline state, deep link reload. No overflow or hidden critical actions.
- Landing copy must never promise 'hands-off', 'self-healing' or 'fully automatic' without current proven claim. Inspect #915 and derive evidence from claims. Project-specific detail belongs one click away; initial page communicates outcome in human language.
- All route/schema docs and examples generated or validated through existing registry. Preserve original canonical backend as authority.

## Acceptance
Run real browser automation with screenshots/trace across widths; test all critical interactions and authorization refusals. Provide recording/artifacts if existing infra supports. Demonstrate a novice path and a power-user path in same UI. No visual-only mock pass.
