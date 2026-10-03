# PROMPT 12 — End-to-end integration, deployment, verification and cleanup

## Mission

Finish the landing transformation completely.

Do not stop at locally rendered HTML.

## End-to-end scenario

Exercise the real supported development journey:

```text
open Cockpit landing
↓
see current repository/runtime state
↓
inspect critical attention
↓
resume active session OR choose ready issue
↓
start canonical action/workflow
↓
observe live state/activity
↓
inspect tests/gates/diff as available
↓
return to landing
↓
see updated canonical state without manual synchronization
```

## Verify important sections

Ensure useful behavior for:

- attention
- continue work
- milestones
- ready work
- active sessions
- active peers
- runtime health
- recent activity
- actions/command palette
- completion gate summaries

Sections with no useful content should collapse/disappear gracefully.

## Cross-surface verification

Check relevant CLI/API/OpenAPI/MCP outputs against Cockpit semantics.

## Build and test

Run all canonical repository gates required by project policy.

Include:

- formatting
- compile
- unit tests
- integration tests
- LiveView tests
- API/OpenAPI tests
- MCP tests
- docs build
- generated artifact checks
- schema validation
- lint/static analysis
- coverage expectations

Use actual repository commands.

## Deploy

If Cockpit/docs are expected to be deployed in this repository workflow:

- execute canonical deployment
- verify deployment success
- verify landing behavior after deploy
- verify GitHub Pages/docs generation if applicable

Never equate "build passed" with deployed/verified.

## Cleanup

Search repository-wide for obsolete artifacts:

- old landing components
- old hardcoded cards
- duplicate metrics queries
- frontend-only registries
- dead CSS
- unused JS hooks
- obsolete API payloads
- stale docs/screenshots
- redundant tests preserving old behavior

Remove safely superseded code.

## Final report

Provide:

### Root cause
Why the old landing page was poor architecturally and operationally.

### Architecture
What now canonically owns landing data and actions.

### UX
What the landing page now enables.

### Interactivity
Realtime/action behavior.

### Cross-surface parity
CLI/API/OpenAPI/MCP/Cockpit/docs.

### Governance
Rules/doctrines/validators preventing drift.

### Tests
Exact relevant evidence and commands.

### Deployment
What was deployed and how it was verified.

### Files changed
Grouped by architectural purpose.

### Removed legacy
Obsolete implementations deleted.

### Remaining debt
Only genuine limitations with reasons.

## Final acceptance invariant

The Cockpit landing page is a useful real-time development control plane backed by canonical Majordomus state and capabilities, not a decorative dashboard.

It is tested, deterministic, documented, governed, cross-surface compatible, deployed where required, and verified with repository evidence.
