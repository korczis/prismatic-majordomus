# Majordomus UI — executable implementation prompt pack

**Repository:** https://github.com/korczis/prismatic-majordomus  
**Target:** `majordomus ui` as the primary human-facing, high-level control plane, derived from existing canonical Majordomus capabilities, also usable in injected consumer projects.  
**Consumer:** Claude Code Opus/Fable; other strong coding agents can use the same pack.  
**Status:** Implementation instructions, NOT proof that implementation already exists.

## Start
In a checked-out repository, give the agent `prompts/00-ORCHESTRATOR.md` followed by `contracts/GLOBAL_CONTRACT.md`, or concatenate them. It must read the rest of the prompts from this directory and execute milestones in dependency order. If the pack sits outside the repo, supply its directory path to the agent. No command in this README pretends that a bespoke `majordomus prompt` subcommand already exists.

## Milestones
| File | Goal | Dependency |
| --- | --- | --- |
| 01 | Repository audit, decision records, executable coverage matrix | none |
| 02 | Canonical application projection and typed query/command API | 01 |
| 02A | **Eliminate manually synchronized inventories, migrate legacy copies, enforce canonical zero-registration propagation** | 01-02 |
| 03 | Installer, direnv, multi-repo launcher and safe local server | 02A |
| 04 | Accessible design system, information architecture and responsive shell | 02-03 |
| 05 | Overview, health, intent and task workflows | 04 |
| 06 | Mesh, peers, tasks, sessions and interaction | 02,04 |
| 07 | Governance, knowledge, quality, evidence and operations | 04-06 |
| 08 | Cross-surface integration, security, UX and E2E hardening | 03-07 |
| 09 | Self-dogfood, CI, docs, release and deployment proof | 08 |
| 10 | Adversarial acceptance audit, repair and closure | 09 |

## Non-goals
No parallel UI state engine, invented agent protocol, unguarded `POST /shell`, fake demo success, hardcoded dashboards, expensive polling on every entry, silent installers, admin-free remote binding, or vanity redesign of the public landing page in place of implementing the application.

## Explicit design decisions to validate, not assume
Flowbite/Tailwind + Alpine.js for modest UI interactivity; ECharts for numerical charts; Cytoscape.js for true graph relationships. Reuse current dependencies and bundler when practical. Server-rendered/progressive enhancement is preferable if architecture supports it. Static assets must work offline after installation if the app is local-first. Existing Cockpit can be adapted or refactored instead of creating an independent UI service.

## Definition of done
`majordomus ui` launches reliably in the Majordomus checkout and injected consumer repositories, displays actual canonical state with provenance and useful high-level information, supports safe end-to-end operations, shows live/observed mesh and sessions truthfully, is usable on 320-390px devices and desktop, honors CLI/API/MCP parity, passes security/UX/accessibility/contract/E2E and release checks, and is actually verified after deployment where credentials allow.

## Newly enforced: no manually synchronized catalogs
Milestone **02A** is obligatory, not optional polish. It audits all existing handwritten copies (CLI/Just/env/API/OpenAPI/MCP/Cockpit/UI/docs/knowledge/mesh/ops), classifies authoritative declarations versus redundant projections, removes duplicates, establishes deterministic canonical presentation ordering, and wires a structural anti-drift gate into the existing local/CI check path. Demonstrate that a newly declared entity appears automatically in all eligible surfaces in both the Majordomus self-repo and a consumer repo, without editing those consumers. Auto-discovery does not imply permission to execute.
