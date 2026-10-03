---
id: project.an-intent-outlives-its-sessions
version: 1
kind: rule
title: An intent outlives the sessions and providers that realise it, and closed work does not outrank evidence
description: Which work realises an intent is derived from the ledger, the session records and the peer board, with every link's provenance; no session, provider or handover holds intent state; and an intent whose work is all closed while its evidence says no is refused by a gate.
statement: The lineage from an intent to the tasks, episodes, providers, handovers, session records and peer claims realising it is derived on read and stored in none of them; every link says whether it is declared, observed, derived or inferred; every episode records its provider on the ledger so the lineage survives the episode's end; and an intent whose milestones are all DONE while a criterion's recorded evidence is failing or stale fails the intent-realization gate, naming the criterion.
status: active
class: blocking
depends_on: [project.work-serves-a-declared-intent@1, project.derived-once@1]
tags: [intent, session, handover, provenance, evidence]

x-majordomus:
  tests: [test/cases/388_an_intent_is_realised_across_providers_and_held_to_reality.sh, apps/majordomus-cli/tests/intent_realization.rs]
---

# Rationale

An intent says what must become true and the plan says which issues realise it, but the work
itself happens in episodes: a Claude Code window starts a task, hands it over, and a Codex
window finishes it. If the answer to "what is this session realising" lived in the session, it
would end with the session; if it lived in a field an agent fills in, it would be as good as the
agent's memory. And if "the work is closed" were allowed to stand for "the intent is true", a
regression after closure would never be seen.

Every fact the lineage needs is already recorded by something that does not depend on the
worker remembering it: the ledger stamps each task, handover and plan transition with its
episode, the episode's start line names its provider, a closed session record lists the issues
it moved, and a peer claim names its scope. The join over them is the lineage, and it is only
honest if a link guessed from overlapping paths is never shown as a link a person declared.

# Required behaviour

- `intent_realization.work` (`majordomus intent realization`, `GET /api/v1/intents/realization`,
  `majordomus_intent_realization`) joins every ledger task, closed session record and peer claim
  to the intents it realises, through issue and milestone, and writes nothing.
- Each link carries `via` and `provenance`: `declared` when the work cites the issue, `observed`
  when its episode moved the issue, `derived` when its branch names the issue, `inferred` only
  when nothing stronger exists and an open issue's scope overlaps. One link per issue, the
  strongest.
- `session.started` carries `provider` and `provider_session` when a provider opened the
  episode, so a closed episode keeps its provider.
- Each intent reports its unmet criteria with the issues serving each, the work and providers
  realising it, and its drift: `closed_work_contradicted`, `closed_work_unproven`,
  `criterion_closed_unmet`. Live work serving no intent is the warning `work_serves_no_intent`.
- `intent_realization.explain` (`majordomus intent explain <id>`) states why the intent stands
  where it stands, sentence by sentence, from the same derivations.
- The `intent-realization` gate runs `majordomus intent realization`, which exits 10 on any
  `closed_work_contradicted`.

# Failure behaviour

`majordomus intent realization` prints each finding with its level, code, subject, message and
reproduce command and exits 10 when an intent whose milestones are all DONE has a criterion whose
recorded run is failing or stale. An intent that is not there is refused by name, never answered
empty.

# Verification

`test/cases/388_an_intent_is_realised_across_providers_and_held_to_reality.sh` drives the loop
through the lifecycle: a Claude Code episode through its own session hook and a Codex episode
through the session entry point carry one task across a handover; closing every issue and the
milestone while one case fails leaves the intent `verifying` with exit 10; the fix satisfies it;
breaking the behaviour takes the satisfaction away again; the repair restores it; and the intent
file is byte-identical throughout. `apps/majordomus-cli/tests/intent_realization.rs` proves the
provenance of each kind of link and that the command line, HTTP and MCP answer the same join.
