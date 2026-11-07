# Baseline and verified sources (snapshot, NOT future truth)

As inspected on 2026-11-07, GitHub `master`: `d7fd9a3617024a7390ac36c1888d6fcee63b600e`. Recheck current master, issues and PRs before work.

## Canonical code/docs
- GitHub repo: https://github.com/korczis/prismatic-majordomus
- IDD workspace: https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/ui/idd.rs
- Capability `intents`: https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/capability/builtin/intents.rs
- Plan capability: https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/capability/builtin/plan.rs
- Source context: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/README.md
- Docs: https://github.com/korczis/prismatic-majordomus/blob/master/docs/CAPABILITIES.md
- Planning: https://github.com/korczis/prismatic-majordomus/blob/master/docs/PLANNING.md
- Reasoning: https://github.com/korczis/prismatic-majordomus/blob/master/docs/REASONING.md
- Owners' pack policy: https://github.com/korczis/prismatic-majordomus/blob/master/docs/OWNER_PACKS.md
- Proceed: https://github.com/korczis/prismatic-majordomus/blob/master/docs/PROCEED.md

## Existing intent contracts
- `the-owner-directs-work-from-the-ui`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/the-owner-directs-work-from-the-ui.yaml
- `a-brief-is-reconciled-into-the-plan`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/a-brief-is-reconciled-into-the-plan.yaml
- `what-a-workspace-decides-reaches-the-repository-with-its-source`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/what-a-workspace-decides-reaches-the-repository-with-its-source.yaml
- `intent-planning`: https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/project/intents/intent-planning.yaml

## Existing issue dependency graph, do NOT replicate
- I3251 #1588: brief grounded in existing project evidence.
- I3252 #1589: in-memory `intents.draft_check` + `intents.draft_write` checked canonical write; open, derived BLOCKED at inspection; depends on I3251 and I3247.
- I3254 #1591: duplicate/overlap detection, open, derived BLOCKED; depends on I3251.
- I3272 #1604: campaign revision/source digest; open, BLOCKED; proposed ADR dependency.
- I3273 #1605: content-derived requirement identity/reconciliation; open, BLOCKED; depends on I3272, I3252, I3251, I3254, I3280, I3281, I3278.
- I3274 #1606: changed requirement segment diff; BLOCKED.
- I3275 #1607: campaign provenance in intent inspectors; BLOCKED.
- I3281 #1612 through I3286 #1617: workspace reference and remote access; several BLOCKED, significant auth/provenance policy prerequisites.
- I4484 #2872: display intent gap/critique in UI; open, BLOCKED.

Important: Issues' `open` state on GitHub is NOT authoritative plan status; Majordomus derives READY/BLOCKED/VERIFY/DONE from canonical records and evidence. These observations are hints for collision avoidance, not a license to overwrite dependency ordering.

## Critically different concepts
1. **Draft (private)** is mutable working state under an existing private checkout/workspace store, not a repository intent or a plan status.
2. **Intent (canonical)** is a typed tracked record whose stage/verdict are derived.
3. **Campaign** is preserved external source/provenance, not an alias for raw chat logs or AI output.
4. **Outgoing prompt pack** is a derived instruction artifact; the current `pack` capability inspected on master shards the repository into bounded text for retrieval. Do not conflate them.
5. **Reasoning** records evidence-based engineering decisions; a model recommendation does not authorize writing a plan.
6. **GitHub Pages** is a published static projection; interactive draft/writes require a trusted runtime.

## Usage note
This pack is source-grounded but cannot know which work has landed after its snapshot; therefore each task explicitly requires current master, active scopes, tests, policy and dependencies to be rechecked before making changes.
