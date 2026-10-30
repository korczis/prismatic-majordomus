---
schema: knowledge/v1
id: the-cognitive-fabric-reuse-map
kind: knowledge
class: fact
title: "Every requirement of the Cognitive Fabric pack already has an owner in the repository; what is missing is reconciliation between owners, and the gaps are listed here"
description: "Before planning or building anything the 2026-10-19 Cognitive Fabric pack asks for, read which module owns it and what it lacks, so that no second implementation is started"
status: verified
epistemics: inferred
date: 2026-10-19
tags:
  - fabric
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:lib/capture.sh
    - file:lib/start.sh
    - file:lib/knowledge.sh
    - file:lib/session.sh
    - file:apps/majordomus-cli/src/entity.rs
    - file:apps/majordomus-cli/src/capability/builtin/entity.rs
    - file:apps/majordomus-cli/src/capability/model.rs
    - file:apps/majordomus-cli/src/capability/builtin/intents.rs
    - file:apps/majordomus-cli/src/capability/builtin/knowledge_base.rs
    - file:apps/majordomus-cli/src/reasoning/policy.rs
    - file:apps/majordomus-cli/src/reasoning/store.rs
    - file:apps/majordomus-cli/src/web/report/snapshot.rs
    - file:apps/majordomus-cli/src/ui/idd.rs
    - file:.github/workflows/pages.yml
    - file:docs/REASONING.md
    - file:docs/MESH.md
    - file:docs/CAPABILITIES.md
    - file:.ai/repo/skills/evolve/SKILL.md
---

# Every requirement of the Cognitive Fabric pack already has an owner in the repository; what is missing is reconciliation between owners, and the gaps are listed here

Compiled from the wave 02-05 reuse matrices and from the code lines a plan author re-read on branch docs/the-owner-briefs-of-10-19-are-planned. Each figure an audit took is given with the command that recounts it, not as a number. The discovery, enforcement and delivery waves (01, 06, 07) were not part of this audit.

Events and intents (wave 02). Hook ingestion: owned by .claude/settings.json and lib/capture.sh; verified (it fails open and never exits 2); it only archives. Idempotency key: lib/capture.sh:563-570; partial, because the bare prompt id is checked over today's files only, with no lock (I3406). Recount shared ids with grep -h '"id":' .ai/local/prompts/*.json | sort | uniq -d | wc -l. Private reads: every read is admitted remotely (capability/model.rs:708-710, ADR 0126); I3434 gates the prompt reader. Causal parent: task.started names no prompt (lib/start.sh:158); I3411. Affected entities: entity edges, intent_binding covers and devcontext select exist, and nothing composes them (I3407, I3412). Match before propose: absent at prompt time (I3408, after I3251). Vertical slice: absent (I3409). Hook latency: I3304.

Knowledge (wave 03). observe, derive, candidates, reject, check, knowledge_on_end and evolve: verified (cases 564, 707-711, 980, 981, 3004). Dedup against curated and pending: full id only (I3413); rejected subjects belong to I3164. Near-duplicates and contradictions: no producer; I3414 reads the reverse references from entity.affected. Promotion evidence: any prose is accepted (I3415). Review as a signal: absent (I3417). Repetition, partial tasks and races hold by construction, untested (I3416).

Reasoning and mesh (wave 04). Plan, consult, record, CI with zero advisors, fallback, circuit breaker and disagreement by evidence: verified (cases 730-736). Source diversity: broken, because selection counts adapters (policy.rs:356-358) while conclusions count advisor ids (store.rs:789-791, 841-843); I3418 and I3419. Peer critique carries a subject line only (I3420). Budgets are an advisor count only (I3421 after I3295; I3422 after I3292). Routing levels: documented as PlanOutcome, with no tier noun (I3423, rule project.no-new-nouns). Hook paths: clean today and unenforced (I3424). Reuse validity: none (I3425). Poisoned answers: untested (I3426).

Surfaces (wave 05). Epistemic class: partial vocabularies, none on EntityView, and proof is a pointer by design (entity.rs:181-185); I3427 never computes verified. Observation envelope: absent on the entity read (I3428, reusing repository.info's shape). Four-surface entity test: absent (I3429). Criterion evidence: landed in I2789's /ui/idd standing panel; knowledge per criterion is absent (I3430). Records written outside the server reaching open views: absent (I3431; reuse crate::change::Generation from feature/the-blackboard-is-pushed when it lands). Snapshot privacy: the gates run before the snapshots are placed (I3432), and the snapshot subjects use route lists, not reading lists (I3433, I3435).

Absent on this branch: the change module of the unlanded Peer Blackboard branch (feature/the-peers-screen-follows-the-blackboard) and http/follow.rs, which exist only on unmerged branches.

## Sources
