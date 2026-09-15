---
id: github-workgraph-phase-01
phase: 1
depends_on: [github-workgraph-shared-contract]
goal: evidence-based gap audit and implementation blueprint
---

# Phase 01 — Discovery, Gap Audit, and Executable Blueprint

Read `00_SHARED_CONTRACT.md` first and obey it.

## Objective

Determine what the repository ACTUALLY has today for:

- milestone/outcome modeling;
- issue/execution-contract modeling;
- dependencies and readiness;
- GitHub connectivity;
- GitHub issue/milestone/PR/check handling;
- branch/worktree ownership;
- commit association;
- acceptance evidence;
- reconciliation/sync;
- CLI/API/OpenAPI/MCP/Cockpit surfaces;
- docs/schema/generation;
- rules/doctrines/policies and CI enforcement.

Do not implement from assumptions. Build a precise map and then immediately land the foundational corrections that unblock the next phase if they are small and unambiguous.

## Required inspection

Search code, tests, docs and generated artifacts for terms and semantic equivalents of:

`github`, `issue`, `milestone`, `work item`, `task`, `execution contract`, `dependency`, `ready`, `blocked`, `branch`, `worktree`, `commit`, `pull request`, `pr`, `check run`, `workflow run`, `acceptance`, `evidence`, `sync`, `reconcile`, `projection`, `provider`, `external id`, `remote`, `capability`, `doctor`, `finish`, `usecase`, `audit`.

Inspect GitHub-related configuration, Actions workflows, issue/PR templates, labels/milestones configuration if represented in repo, and any code that shells out to `git` or `gh`.

Inspect current public/external GitHub state only through established tooling and only as needed. Do not infer canonical architecture merely from what happens to exist remotely.

## Produce an architectural inventory

Create a temporary/internal audit matrix with columns equivalent to:

| Concern | Canonical source today | Model/type | Persistence | Derived from | Mutated by | Consumers | Enforcement | Tests | Drift risk | Action |
|---|---|---|---|---|---|---|---|---|---|---|

At minimum include:
- milestones;
- issues;
- dependencies;
- status;
- GitHub external identity;
- labels;
- assignees if supported;
- branch;
- worktree;
- commits;
- PR;
- reviews;
- CI/checks;
- acceptance evidence;
- completion;
- docs;
- Cockpit state.

## Find duplication

Explicitly identify:
- the same issue/milestone data stored both locally and on GitHub;
- repeated lists of commands/routes/tools;
- hardcoded UI navigation/inventories;
- manual docs tables;
- shell scripts implementing domain logic;
- string parsing of branch/issue relationships;
- GitHub calls hidden in renderers;
- status fields stored when they should be derived;
- generated files accidentally treated as editable authority.

## Classify each relation

For every relationship, decide whether it is:
- canonical semantic data;
- Git-derived;
- GitHub-observed;
- projected to GitHub;
- inferred;
- computed;
- cached;
- historical/audit evidence.

Example question: "Issue belongs to milestone." Is that canonical local semantics projected to GitHub, GitHub-owned semantics imported into local state, or mergeable? Do not leave this ambiguous.

## Establish naming/identity facts

Find the existing canonical ID patterns. Determine how a work item can be referenced without relying on mutable GitHub numbers. Determine repository identity semantics across forks/remotes.

Find whether branch naming already encodes issue IDs. Do NOT require such encoding unless existing doctrine says so; association may be explicit/derived another way.

## Build a dependency graph of implementation work

Before broad edits, determine the minimal sequencing. Expected conceptual order:

1. typed canonical graph/model;
2. external identity/provenance;
3. GitHub provider;
4. reconciliation;
5. Git/worktree/commit/PR evidence;
6. governance;
7. capability projections;
8. Cockpit;
9. migration;
10. E2E/CI/audit.

Adjust to actual architecture.

## Deliverables

Commit an evidence-based architecture/gap document in the repository ONLY if current documentation conventions have a suitable canonical location and such an audit is intended to persist. Otherwise keep the audit in session/handover and update durable docs with decisions, not ephemeral notes.

If a formal ADR is required by current policy for the Work Graph/reconciliation architecture, prepare the ADR structure but do not prematurely freeze details that Phase 02/03 must validate.

## Gate

Do not proceed conceptually until you can answer:

1. What is canonical?
2. What is derived?
3. What is Git-owned?
4. What is GitHub-owned?
5. What is projected?
6. What state is currently duplicated?
7. Where can drift occur?
8. Which existing capability/generation mechanism must own new operations?
9. Which existing rule/doctor/check/finish mechanisms will enforce it?
10. Which legacy objects will need backfill?

Run repository checks relevant to any changes made in this phase and create the normal handover.
