---
schema: adr/v1
id: adr-0214
kind: adr
title: "A provider's native extension points carry the contract as generated projections; Majordomus neither embeds nor patches the worker's runtime"
status: proposed
date: 2026-10-18
tags:
  - providers-economics
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0024-an-orchestrator-is-a-provider-only-at-the-bootstrap-level-an.md
  - file:.ai/repo/adrs/0136-every-session-briefs-itself-on-an-interval.md
  - file:.ai/repo/adrs/0140-editors-and-agents-read-projections-of-the-layer-and-cargo-e.md
  - file:.ai/repo/adrs/0146-a-capability-is-invoked-by-name-through-a-generated-skill.md
  - file:.ai/repo/adrs/0165-a-skill-is-derived-for-every-domain-of-the-registry.md
  - rule:project.a-standing-approval-is-written-where-permissions-are-read
  - file:.ai/repo/project/issues/I3300.yaml
  - file:.ai/repo/project/issues/I3301.yaml
  - file:.ai/repo/project/issues/I3302.yaml
  - file:.ai/repo/project/issues/I3303.yaml
  - file:.ai/repo/project/issues/I3308.yaml
  - file:.ai/repo/project/milestones/a-provider-carries-the-contract-natively.yaml
  - file:.ai/repo/project/intents/a-provider-carries-the-contract-natively.yaml
  - file:.ai/repo/knowledge/curated/claude-code-mods-are-unsandboxed-hooks-in-plugins.md
  - file:.ai/repo/knowledge/curated/a-worktree-is-not-a-security-boundary.md
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0024-an-orchestrator-is-a-provider-only-at-the-bootstrap-level-an.md
    - file:.ai/repo/adrs/0136-every-session-briefs-itself-on-an-interval.md
    - file:.ai/repo/adrs/0140-editors-and-agents-read-projections-of-the-layer-and-cargo-e.md
    - file:.ai/repo/adrs/0146-a-capability-is-invoked-by-name-through-a-generated-skill.md
    - file:.ai/repo/adrs/0165-a-skill-is-derived-for-every-domain-of-the-registry.md
    - file:.ai/repo/project/issues/I3300.yaml
    - file:.ai/repo/project/issues/I3301.yaml
    - file:.ai/repo/project/issues/I3302.yaml
    - file:.ai/repo/project/issues/I3303.yaml
    - file:.ai/repo/project/issues/I3308.yaml
    - file:.ai/repo/project/milestones/a-provider-carries-the-contract-natively.yaml
    - file:.ai/repo/project/intents/a-provider-carries-the-contract-natively.yaml
    - file:.ai/repo/knowledge/curated/claude-code-mods-are-unsandboxed-hooks-in-plugins.md
    - file:.ai/repo/knowledge/curated/a-worktree-is-not-a-security-boundary.md
---

# 214. A provider's native extension points carry the contract as generated projections; Majordomus neither embeds nor patches the worker's runtime

## Context

Claude Code gained plugins and mods (dated 2026-10-02); the owner asked whether Majordomus could be replaced by plugins (10-08) and to remove the client's brakes, including by patching (10-16). The repository already ships two mods (ADR 0136), a generated skill per domain (ADR 0165) and a PreToolUse edit guard. README carries no 'does not hook the worker' statement to amend.

## Decision

Majordomus owns intent, scope, state, evidence, finish and the mesh; a client's hooks, skills, plugins and mods are execution mechanisms it projects onto, generated from the registry and drift-gated. It builds no agent runtime, marketplace or scheduler, patches no client binary, and projects no permission mode; friction is reduced by settings rules a person writes.

## Alternatives rejected

- Reimplementing generic agent workflow inside Majordomus
- Patching or forking a client
- A Majordomus marketplace

## Consequences

Milestone a-provider-carries-the-contract-natively; the direct-call agent adapter interface proposed 2026-09-12 is withdrawn.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c174#18`
- `chatgpt-majordomus:c193#0`
- `chatgpt-majordomus:c194#3`
