---
schema: adr/v1
id: adr-0070
kind: adr
title: An intent is a typed record, and its stage and satisfaction are derived from the plan and the evidence
status: proposed
date: 2026-09-15
tags:
  - intent
  - project
  - evidence
  - governance
related:
  - rule:project.work-serves-a-declared-intent
  - rule:project.derived-once
  - rule:project.no-claim-without-test
  - file:share/schemas/majordomus/intent/intent.v1.schema.json
  - file:apps/majordomus-cli/src/intent.rs
  - file:apps/majordomus-cli/src/capability/builtin/intents.rs
  - file:.ai/repo/project/intents/intent-lifecycle.yaml
  - file:.ai/repo/project/milestones/intent-lifecycle.yaml
  - test:test/cases/367_an_intent_is_satisfied_only_by_evidence.sh
provenance:
  origin: authored
---

# 70. An intent is a typed record, and its stage and satisfaction are derived from the plan and the evidence

## Context

The project model says what work exists: milestones are outcomes, issues are execution
contracts, and `plan.rs` derives every status from what happened to them. The evidence
fabric says what ran: `docs/CLAIMS.yaml` names a test, and the ledger holds the run. Neither
says what the work was for. The public site said so plainly — there was no intent object, no
intent registry and no intent id — and every question of the form "which outcome does this
change serve" or "did finishing these milestones achieve anything" was answered, if at all,
from a prompt that did not survive its session.

Two ways to add the missing layer were open. One gives an intent a lifecycle of its own:
a status field, transitions, a stamp when a pull request merges. The other treats an intent
as a statement whose standing is computed from records the repository already keeps.

## Decision

An intent is a YAML record under `.ai/repo/project/intents/`, kind `intent`, schema
`majordomus.intent/v1`: an `id`, a `title`, the `statement` that must become true, the
`invariants` that must stay true, the `milestones` that realise it, and `satisfaction`
criteria, each `{id, criterion, evidence, ref}` with `evidence` one of `test`, `claim`,
`command` or `deployment`. Optional `governance`, `non_goals`, `cancelled` and
`superseded_by`. The kind is declared in the distribution's `share/kinds.yaml` and its class
in both this repository's and the skeleton's `sources.yaml`.

**Intent is not a second task or lifecycle model.** No stage is stored, no capability
transitions an intent, nothing stamps one, and no intent capability writes the repository.
The stage is a pure derivation over two things that already exist:

- the status `plan::Plan` derives for each named milestone — `declared` while a milestone
  does not resolve, `planned` once all do, `executing` once any is ACTIVE, VERIFY or DONE,
  `verifying` once every milestone that is not cancelled or superseded is DONE;
- the proof state the evidence fabric already derives — a `test` criterion is met by a
  passing ledger run whose source digest still matches, a `claim` criterion by a claim that
  is `proven` or `inputs unchanged`. `satisfied` is `verifying` with every criterion met.

`command` and `deployment` criteria resolve, and are never counted as met: there is no
recorded execution to derive them from, and treating a declared command as proof would be the
anonymous green the ledger exists to refuse.

Four read-only capabilities project the derivation to the command line, HTTP and MCP:
`intents.list`, `intents.record`, `intents.validate` (exit 10 on a failure) and
`intents.preflight` (issue or paths, to milestone, to the intents it serves, or a refusal naming
the missing link). The `intent-check` gate runs `intent validate` on a change to the project
model or the engine, and `project.work-serves-a-declared-intent` states the rule.

## Alternatives rejected

**A stored intent status with transitions.** It would be a second lifecycle beside the plan's,
and the two would disagree the first time an issue was re-planned — the defect the plan was
written without a status field to avoid.

**Satisfaction stamped when the last pull request merges.** A merged pull request whose
intent is still false is unfinished work; the git server is not an oracle of the outcome.

**A new evidence ledger for intents.** The ledger and the claim join already say what ran and
whether it is current. A second one would be a second truth about the same executions.

## Consequences

- Satisfaction is currently decided by the intent engine over the ledger. Completion policy
  (pull request 292, ADR 0057, not yet on master) makes whether work is done a policy question;
  once it lands, whether an intent is satisfied becomes a completion-policy question over the
  same evidence, and this engine answers the stage only. That is slice 2 of the
  `intent-lifecycle` milestone.
- `milestone_serves_no_intent` is a warning, so existing milestones stay valid while intents are
  written for them.
- The devcontext and session preflight join, the Cockpit intent view with enforced script
  coverage, and GitHub milestone reconciliation with a closure gate are later slices and are
  not decided here.
