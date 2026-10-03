# Prompt 11 — Documentation, Generated Reference, and GH Pages Auto-Sync

## Mission

Make documentation and GH Pages a continuously derived explanation of the real system.

No manually updated "feature list" that becomes fiction one sprint later.

## Audit

Find:
- docs source hierarchy;
- generated docs;
- registry reference;
- GH Pages source/build;
- site navigation;
- capability pages;
- plan/milestone pages;
- changelog;
- API reference;
- Swagger links;
- Cockpit docs;
- deployment workflow;
- drift checks.

## Required generated content

Where machine-derivable, site/docs should automatically expose:

- current version/build;
- capability registry;
- modules;
- transport exposure matrix;
- CLI reference;
- API/OpenAPI reference;
- MCP surface;
- Cockpit feature/route catalogue;
- workflows;
- rules/doctrines/policies index;
- ADR index/graph;
- use cases;
- schemas/kinds;
- benchmark coverage;
- health/architecture diagrams where deterministic;
- issue/milestone plan integration if project policy treats it as publishable;
- changelog/releases.

Do not manually copy these lists into prose.

## Cockpit documentation

Update canonical docs to explain:
- Cockpit as projection;
- IDE workspace;
- execution console;
- workflow launch;
- peers/sessions;
- governance;
- planning/git;
- observability;
- API/Swagger relationship;
- security model;
- extension model.

## GH Pages

Ensure:
- site builds from canonical generated artifacts;
- `majordomus generate --check` or current equivalent detects drift;
- CI builds/tests site;
- deployment is automatic according to repo policy;
- broken links are tested;
- version/capability counts are derived;
- Swagger/API/Cockpit links are environment-aware rather than hardcoded localhost where inappropriate.

## Synchronization invariant

A new capability/workflow/rule should require changing only its canonical source. After generation it should appear in all applicable docs/site indexes.

Add a regression test for this.

## Deployment verification

After docs/site deployment:
- verify workflow success;
- fetch/check public GH Pages routes if repository workflow permits;
- verify representative generated page;
- verify no stale previous version;
- verify links.

Record exact deployment evidence in final report.
