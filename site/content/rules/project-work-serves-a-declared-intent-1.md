+++
title = "Work serves a declared intent, and an intent is satisfied only by evidence"
description = "Work serves a declared intent, and an intent is satisfied only by evidence"
weight = 162
[extra]
kind = "rule"
slug = "project-work-serves-a-declared-intent-1"
identity = "project.work-serves-a-declared-intent@1"
status = "active"
source = ".ai/repo/rules/project/work-serves-a-declared-intent.v1.md"
+++
{% raw %}

## Rationale

The plan says what work exists and in which order; the evidence ledger says what ran. Neither
says what the work was for. That question lived in prompts, and a prompt is unversioned,
unenforced and gone at the end of a session, so no command could say which outcome a change
served or whether finished milestones achieved it.

An intent record closes that gap only if it does not become a second account of the work.
A stored intent status would drift the first time an issue was re-planned, exactly as a
stored issue status would, which is why the plan stores none.

## Required behaviour

- An intent lives under `.ai/repo/project/intents/<id>.yaml` and is validated by
  `majordomus.intent/v1`; its keys are closed by `share/allow/intent.txt`.
- An intent names at least one milestone and at least one satisfaction criterion, each
  criterion naming its evidence by kind and reference.
- The stage is derived from the status the plan derives for the named milestones, and a
  criterion is met only when the evidence ledger holds a current passing run for its test,
  or its claim is proven or has unchanged inputs. `command` and `deployment` evidence
  resolves but is never counted as met.
- No capability writes an intent, stores its stage or transitions it.
- `majordomus intent validate` exits 10 on any failure, and the `intent-check` gate runs it
  on every change to the project model or the intent engine.

## Failure behaviour

`majordomus intent validate` prints one line per finding with its level, code, subject,
message and reproduce command, and exits 10 when any is a failure: `intent_without_milestone`,
`intent_without_criterion`, `unknown_milestone`, `criterion_without_ref`,
`unresolved_evidence_ref`, `unresolved_governance`, `unknown_successor`, `duplicate_criterion`,
`file_name_mismatch`. A milestone no intent serves is the warning
`milestone_serves_no_intent`.

## Verification

`apps/majordomus-cli/tests/intent.rs` proves the command line, HTTP and MCP answer the same
derived intents and that an invalid model exits non-zero naming the finding.
`test/cases/367_an_intent_is_satisfied_only_by_evidence.sh` proves through the built
executable that a recorded passing run is what moves a criterion to met, and that a broken
reference is refused.
{% endraw %}
