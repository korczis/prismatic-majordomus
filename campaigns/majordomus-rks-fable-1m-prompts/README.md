# Majordomus Repository Knowledge System (RKS) — Claude Code Fable 1M Integration Pack

This pack is designed for immediate use against the `prismatic-majordomus` repository with Claude Code Fable using a very large context window. The prompts are intentionally staged. Do not collapse them into one mega-run unless you want to trade determinism for theatre.

## Goal

Implement a production-grade, brownfield-capable Repository Knowledge System (RKS) that can discover, derive, validate, reconcile, expose, document, and continuously keep repository knowledge in sync without manual registries or duplicated sources of truth.

RKS must fit Majordomus principles:

- inferred / derived / discovered where possible
- explicit declaration only where inference is impossible or unsafe
- one canonical machine-readable definition per capability
- no duplicated manual registries
- data-driven and schema-first
- extensible via typed capability discovery
- deterministic-first, LLM-assisted second
- provenance-aware and evidence-backed
- brownfield-safe via baselines
- worktree-aware and branch-local
- integrated with CLI, API, OpenAPI/Swagger, MCP, Cockpit, docs, completion, rules/doctrines, CI and tests
- dogfooded by Majordomus itself
- migration/version aware from day one
- secure around secrets and remote LLM processing

## How to use

Run the prompts in order. Each prompt assumes the previous phase is either committed or at least locally complete and green.

Recommended invocation pattern:

```bash
cd ~/dev/prismatic-majordomus
claude
```

Then paste one prompt at a time. Let Claude inspect the repository rather than presuming paths or architecture from this pack. The prompts explicitly instruct it to preserve existing conventions and discover the actual repo state first.

### Suggested execution sequence

1. `00_MASTER_CONTRACT.md` — load first in every session or keep pinned as the governing contract.
2. `01_REPOSITORY_AUDIT_AND_ARCHITECTURE.md`
3. `02_DOMAIN_MODEL_SCHEMAS_AND_REGISTRIES.md`
4. `03_BROWNFIELD_DISCOVERY_AND_BOOTSTRAP.md`
5. `04_FRESHNESS_IMPACT_AND_RECONCILIATION.md`
6. `05_CLI_API_OPENAPI_MCP_INTEGRATION.md`
7. `06_COCKPIT_UI_UX_AND_GRAPH.md`
8. `07_RULES_DOCTRINES_CI_AND_DOGFOODING.md`
9. `08_SECURITY_LLM_AND_PROVIDER_POLICIES.md`
10. `09_DOCS_MARKETING_DEMO_AND_PRODUCTIZATION.md`
11. `10_HARDENING_E2E_PERF_AND_RELEASE.md`
12. `11_FINAL_REVIEW_GAP_CLOSURE_AND_SHIP.md`

## Working discipline

Each phase must:

- inspect current repo state and recent relevant git history
- avoid assumptions about file layout
- preserve backwards compatibility unless explicitly justified
- update existing abstractions rather than create parallel ones
- prefer extension of current registries/macros/schema machinery
- never create a new manual registry if discovery can be derived
- produce tests with realistic fixtures
- update docs and exposed interfaces from the same source of truth
- run the strongest available local checks
- show a concise final change inventory and remaining risks

## Branch / worktree discipline

Use the Majordomus repo’s enforced worktree rules. If the repo root is:

```text
~/dev/prismatic-majordomus
```

feature worktrees belong in the inferred sibling root:

```text
~/dev/prismatic-majordomus-wt/<branch-path>
```

Each logical feature must have its own branch and matching worktree. Do not invent a different layout.

## Completion standard

RKS is not done when Markdown exists. It is done when the same canonical model drives the implementation and applicable projections across:

```text
implementation
  ↓
typed model / registry
  ↓
JSON Schema
  ↓
CLI
API
OpenAPI / Swagger
MCP
Cockpit
docs
completion
rules / doctrines
CI / gates
E2E tests
```

The product-level acceptance statement is:

> A pre-existing repository can adopt Majordomus later, bootstrap a knowledge system without destroying or duplicating existing documentation, detect drift as the repository changes, explain why it believes each derived claim, expose that knowledge consistently to humans and agents, and prevent new knowledge debt while tolerating a declared brownfield baseline.
