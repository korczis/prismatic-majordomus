# MASTER INSTRUCTIONS — Cockpit Landing Page Transformation

You are working in the `prismatic-majordomus` repository using Claude Code.

Your mission is to transform the Cockpit landing page into a first-class, high-information, interactive development control plane that is actually useful for daily software development.

Do NOT treat this as a visual facelift.

Treat it as a repository-wide product and architecture correction whose first consumer is the Cockpit landing page.

## Target outcome

When a developer opens Cockpit `/`, the page should immediately answer:

1. What is happening in the repository right now?
2. What requires attention?
3. What work can be resumed immediately?
4. What issues/milestones are ready, blocked, active, or nearly done?
5. Which peers/sessions/workflows are active?
6. Is the runtime healthy and synchronized?
7. Which canonical actions can be executed right now?
8. Are quality/completion gates passing?

The landing page should feel like a development operations console, not a marketing homepage and not a generic admin dashboard.

## Required architectural shape

Prefer this shape, adapting names to the repository's existing conventions:

```text
canonical repository/project/runtime state
        ↓
canonical landing projection / view model
        ↓
LiveView / Cockpit rendering

same canonical state also consumed through:
CLI · JSON · REST/API · OpenAPI/Swagger · MCP · docs
```

Cockpit MUST NOT independently rediscover or reinterpret repository semantics.

## Before every phase

Inspect and obey the repository's actual governance and architecture:

- `AGENTS.md`
- rules/doctrines/policies
- ADRs
- session contexts / handovers
- existing Cockpit architecture
- API/OpenAPI/MCP architecture
- CLI architecture
- issue/milestone integration
- peer/runtime/session architecture
- tests
- generated documentation
- GitHub Pages generation
- deployment workflow

Discover actual paths. Do not assume them.

## Global implementation rules

Everything feasible must be:

- typed
- deterministic
- inferred
- derived
- data-driven
- schema-backed
- introspectable
- documented
- tested
- validated
- automatically discoverable
- reusable across surfaces
- future-proof against new entities

Never solve the task by introducing:

- giant hardcoded HEEx lists
- giant hardcoded JavaScript arrays
- duplicate enums in frontend and backend
- frontend-only sorting semantics
- separate Cockpit-only status definitions
- separate Cockpit-only workflow inventories
- manually synchronized docs inventories
- one-off endpoint payloads duplicating domain models
- fake/demo/synthetic data in production paths

## Definition of done for every phase

Before considering a phase complete:

1. format changed code
2. compile/build affected components
3. run focused tests
4. run affected integration tests
5. run schema validation
6. run generated artifact drift checks
7. run docs validation
8. run relevant CLI/API/MCP/Cockpit tests
9. inspect `git diff`
10. search for legacy/duplicate code made obsolete
11. verify no secrets/local-machine-specific paths were added
12. update documentation
13. update/add governance enforcement if needed
14. commit cleanly according to repository policy
15. push/deploy when repository policy requires it
16. verify actual deployed/runtime behavior when in scope

Do not write "done" if any mandatory evidence is missing.

## UX target

The final page should support, using real data where available:

```text
CONTINUE WORK
ATTENTION REQUIRED
ACTIVE DEVELOPMENT
MILESTONES
READY WORK
LIVE PEERS
RUNTIME / SYNC HEALTH
RECENT ACTIVITY
COMPLETION GATES
COMMAND / ACTION PALETTE
```

Sections should appear only when useful. Empty filler panels are forbidden.

## Interaction target

From the landing page, a developer should be able to reach or execute appropriate canonical actions such as:

```text
resume session
open issue
investigate issue
implement issue
review issue
run tests
inspect diff
inspect failing gate
open milestone
start workflow
inspect peer
request review
open REPL
run command
finish task
```

Actions must derive from canonical capability/workflow availability.

## Final invariant

Majordomus discovers project/runtime facts once, models them once, derives a canonical useful landing projection once, and exposes that projection/underlying domain through all eligible surfaces.

Cockpit renders reality. It does not invent it.
