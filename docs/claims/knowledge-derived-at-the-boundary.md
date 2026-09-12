# A provider's end event and its compaction event derive the knowledge the episode produced into candidate records, from the ledger and git and never from a conversation

## What it means

When an episode ends, and when the conversation inside one is compacted, Majordomus reads the ledger lines the episode stamped — the decisions it recorded, the questions it resolved, the tasks it finished — and writes each one as a candidate knowledge record under `.ai/repo/knowledge/candidates/`. The record is one assertion with a class, a status of `candidate`, an epistemic stance and provenance naming the episode and the task or decision it came from. Nothing reads the conversation, a prompt or a handover, and no model is called. The ledger records that the derivation ran, with the episode, the counts and the paths, even when nothing was written.

## How it works

The close path is one: `mj_session_close` in `lib/session.sh` runs `majordomus knowledge derive` after the session record is published and before the `session.closed` line is appended, whenever `session.knowledge_on_end` is not off. Every close — the provider's end adapter, `majordomus session close` typed by a person, any future caller — is therefore followed by its `knowledge.derived` line. The compaction adapter in `lib/capture.sh` has no close to ride on and calls the deriver itself, switched by `session.knowledge_on_compact` and independent of `checkpoint_on_compact`.

The deriver resolves the episode, selects its ledger window and maps each evidence line to a record by a fixed table: `decision.recorded` becomes a `convention` with epistemics `decided`; `question.resolved` becomes a `fact` that states the question it answers; `task.finished` with `blocked`, `failed` or `no_match` becomes a `lesson` carrying the first line of the task note's reason; `task.finished` with `completed` and a verification command becomes a `fact` naming the command that passed. Commits of the episode that touched a rule or a decision record add `commit:<sha>` to every record's provenance and a `relates_to` relation to each path. A handover's Next Action is continuation state and is never derived from. It runs whether or not a task is active, because the lifecycle is the episode's (ADR 0052).

## How to see it

```bash
majordomus capture install
printf '{"session_id":"e1","source":"startup"}' | .claude/hooks/majordomus-session-start >/dev/null
majordomus start "a task" --scope lib
majordomus decision add "Tabs are refused in the parser" --why "two encodings of one thing"
printf '{"session_id":"e1","reason":"other"}' | .claude/hooks/majordomus-session-end
ls .ai/repo/knowledge/candidates/                       # e1-<digest>.md beside README.md
majordomus history --event knowledge.derived            # episode e1, written 1
majordomus knowledge candidates                         # the record, its class, its branch
```

## What it does not cover

It does not summarise, quote or classify a conversation; the evidence is the ledger, the task notes and git, and nothing else. It does not derive from a handover or a checkpoint, which are projections of the same ledger. It does not write `verified`: every record it writes is a candidate, and promotion is a person's act (`knowledge-promotion-is-an-act`). A decision recorded outside any task derives from the session alone.

## Why it exists

Every decision recorded under a task was visible in `majordomus history`, on one machine, until the ledger's retention cap removed it. The layer already had the object for it — ADR 0010 fixed that curated knowledge is one kind with a class and a status — and no writer. `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` records the decision to derive at the same moments the checkpoint is derived, for the checkpoint's reason: they are the moments at which what the episode knows stops being reachable.
