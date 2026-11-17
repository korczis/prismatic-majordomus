# MASTER PROMPT — Autonomous Public Surface Release Train

You are Claude Opus/Fable operating in `korczis/prismatic-majordomus`. Read `00_README.md` and execute atoms A00–A15 in dependency order. Goal: polished, mobile-first, project-oriented public site + GH Pages + docs, *all derived/inferred/proved*, with automatic `.ai` enforcement and progressive, verified releases.

### Mandatory preflight
- Refresh repo, inspect branch, worktrees, PRs, current CI/deploy, `.ai`, AGENTS/CLAUDE instructions, current metadata/schema and site. Record current HEAD and published commit. Do not assume deployment is clean.
- Locate exact executable gates; verify `scripts/pages` help/implementation and the CI model. Capture baseline durations, asset size, performance, a11y and mobile issues. Read docs/PRODUCT.md and relevant ADRs.
- Register one top-level intent and milestones with acceptance criteria/evidence using actual Majordomus primitives. If the primitives are absent, file this in the existing native work tracking architecture; don't invent a new tracker.
- Produce an ownership/dependency graph from `manifest.json`; run non-conflicting atoms in parallel through the existing mesh only if it demonstrably handles leases, files and handoffs. Otherwise run sequentially. Do not wait for a person on ordinary choices.

### Workflow discipline
For each atom: inspect → plan → implement → negative tests → positive tests → docs → `.ai` generalized rule/gate update where warranted → local verification → merge/release decision → deployed commit verification where applicable → evidence and handover. Stop that atom at a real blocker, isolate it, and continue independent atoms only if safe.

### Constraints
Existing `homepage.toml` is declarative story order, `marketing.toml` is editorial, public inventories/counts live in canonical registries. Existing Flowbite/Tailwind/design tokens and `scripts/pages` are foundations, not disposable legacy. Do not create second content model, handcrafted search catalogue, fake transcripts, separate navigation arrays, or a new CI pipeline. Validate machine-enforced status of every claim, and distinguish built from planned. New features and public objects should propagate with zero consumer-specific registrations. No leaking private/local `.ai`, secrets, addresses or absolute filesystem paths into public assets.

### Release cadence
Deploy after every coherent **public-affecting, validated vertical slice** (e.g. IA+navigation, proof UX, explorer, docs/search, mobile polish) rather than after every tiny commit. For slices merged concurrently, deployment is of their combined source commit. Version bump per *actual* doctrine, not arbitrary minor per edit. Verify live commit and route set. A local build or successful push is not sufficient. Never claim success without links/hashes/logs.

### Completion contract
- All atoms passed or have explicit external blockers, all deliverables checked, no source-of-truth duplication, no regression on existing homepage/prod model constraints.
- Public Pages serves the final merged source SHA, docs and representative routes return expected content, release metadata agrees.
- Demo proof navigable: problem → feature → guarantee → executable evidence → source. One new canonical object propagates into relevant docs/search/nav without manual consumer changes.
- Final evidence report includes per-atom status, source files, test commands/outcomes, `.ai` changed validators/gates, release/version actions, PR and merge SHAs, Pages verified SHA, timings and known blockers. Never conflate 'implemented', 'merged' and 'live'.

Execute A00 first, then the dependency graph. No analysis-only stopping point.
