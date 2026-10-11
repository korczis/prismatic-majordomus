# Majordomus: Intent → Verified Outcome

**Target:** `korczis/prismatic-majordomus` and projects using injected `.ai`.

This pack is an execution program for Claude Code (Opus/Fable or comparable model), not a claim that the features already exist. Start with `00-MASTER-ORCHESTRATOR.md`; each numbered file is one milestone and each `Issue` subsection is one issue. One prompt can be run alone against a repo with Majordomus or the master can execute all in dependency order.

## Operating model

Human outcome → canonical intent → context/gap → plan/opposition → native tasks/mesh/sessions → executable evidence → derived satisfaction → replan or finish.

Do not install a parallel agent framework, duplicate stores, independently maintained API/UI catalogs, or script-only business logic. Inspect the checkout's HEAD and local .ai policies before implementation.

## Known reference gaps (not a live audit)

Public project pages describe typed intent records and derived stage, criterion/issue coverage, realized work joins, a gap/critique validation layer, and CLI/API/MCP surfaces. Pages also explicitly say command/deployment evidence is not implemented and critique validation occurs after plan-start, rather than blocking its transition. These are investigation targets, not an assertion about HEAD when a prompt executes.

Sources:
- https://majordomus.dev/guarantees/intent-stage-derived/
- https://majordomus.dev/guarantees/intent-criteria-covered/
- https://majordomus.dev/guarantees/intent-plan-reviewed/
- https://majordomus.dev/guarantees/intent-command-deployment-evidence/
- https://majordomus.dev/guarantees/intent-realization-held-to-evidence/

## Run

Open the repository in Claude Code, paste `00-MASTER-ORCHESTRATOR.md`. To resume, supply the last completed phase number and rerun the master; it must derive actual progress from repo state, not trust the assertion. Individual milestone prompts are independently usable. No artificial time deadline overrides correctness, authorization or actual evidence.

## Global definition of done

1. Intent satisfaction is determined exclusively by valid, current evidence for every criterion; completed issues alone do not suffice.
2. Work is derived from uncovered criteria and critiques are enforced before starting work, not afterward.
3. The execution loop makes bounded, observable progress and survives interruption without duplicate work.
4. Same typed canonical model across CLI, REST/OpenAPI, MCP, UI, docs and GitHub projections where available.
5. Works both in the platform repo and injected consumer projects, with real tests and zero redundant consumer registration.
6. Honest guarantees, no fabricated evidence, safe external side-effect control, repeatable end-to-end regression proof.

## Files

- `00-MASTER-ORCHESTRATOR.md`: start here.
- `01`–`08`: eight milestones, four granular implementation issues each.
- `ACCEPTANCE-MATRIX.md`: end-to-end acceptance and regression scenarios.
- `EXECUTION-REPORT-TEMPLATE.md`: evidence-oriented handover.
