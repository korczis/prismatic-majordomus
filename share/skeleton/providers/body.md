## How AI work runs in this repository

This project is supervised by Majordomus. Read this once; it is short on purpose.

### Ten rules

1. **Sessions are workers, not memory.** Durable state lives in `.majordomus/state/`,
   never in this conversation.
2. **Load minimum sufficient context.** Your profile says what to load. Do not read the
   whole repository to orient; run `majordomus check --explain`.
3. **Externalise decisions.** A decision you make goes into
   `.majordomus/state/decisions.md`, dated, with what was rejected and why.
4. **One worker, one scope.** Touch only the paths in your task's scope. If the fix is
   elsewhere, stop and record an open question.
5. **Escalate capability and effort only when justified.** The profile sets the
   default. Escalation is recorded, not assumed.
6. **Execution depth is not output verbosity.** Think as hard as the task needs; report
   as briefly as the profile says.
7. **Define done before executing.** `majordomus finish` evaluates the contract below.
   Nothing else counts.
8. **Verify outcomes, not activity.** A test that ran and passed is evidence. A sentence
   saying it did is not.
9. **Parallel work requires isolation.** Do not start a second task in a checkout that
   already has one.
10. **Handovers transfer state, not transcripts.** Never paste conversation history
    into any file.

### Never author identity fields

`repository_id`, `branch`, `head`, `working_tree`, `changed_files` on any state record
are computed by Majordomus from git. Do not write them, guess them, or "fix" them.

### Lifecycle

```
majordomus start "<task>" --scope <paths> [--profile <name>]
majordomus context            # what you need to know now; run this first, every session
   ... work ...
majordomus checkpoint         # progress, on stdin, every {{CHECKPOINT_DEFAULT}}
majordomus check              # before claiming anything is done
majordomus handover < note.md # to continue in another session, or
majordomus finish --outcome <completed|partial|blocked|no_match|failed> --verify-command "<cmd>"
```

Default profile: `{{DEFAULT_PROFILE}}`.

### Start every session with `majordomus context`

Your conversation does not survive. The repository's records do. `context` assembles them —
git state, the task, the profile, open blockers, recent decisions, the newest checkpoint,
the most relevant handover — in authority order and within a line budget. Do not reconstruct
state from a transcript, and do not ask a human to re-explain what a record already says.

Everything it prints is evidence, not authority. It labels how far git has moved since each
record was written: `exact`, `advanced`, `diverged`, `different_context`. On `diverged` or
`different_context`, trust git and say so.

### While you work

| When | Run |
|---|---|
| you learned something the next worker needs | `majordomus checkpoint` (body on stdin, short) |
| you chose between real alternatives | `majordomus decision add "<what>" --why "<why>"` |
| you are blocked on a person | `majordomus question add "<question>"` |
| you need a record you cannot name | `majordomus search "<text>"` |
| you want a reusable framing | `majordomus prompt list`, `prompt render <name>` |

A checkpoint is short by policy and refused if it is long — write a handover instead. An
unresolved question refuses `finish --outcome completed`, which is the point of recording it.

### Profiles

{{PROFILE_TABLE}}

Capability classes are not vendor model names; pick the closest your environment offers.

### Finish contract for `completed`

{{FINISH_CONTRACT}}

Other outcomes need a note with `# Next Action` (partial, blocked) or `# Reason`
(no_match, failed). `no_match` means the work was done and the thing sought does not
exist; `failed` means the work could not be done. They are different facts.

### Handover

Body on stdin with these level-one headings, each non-empty:
{{REQUIRED_SECTIONS}}. Front matter is computed; do not write it. Write a handover when you
stop with work left, not at every pause — that is what a checkpoint is for. No transcript,
no diff, no narrative of the session: what is true now, and the one next action.

### Where things are

| | |
|---|---|
| canonical policy | `.majordomus/policy.yaml` |
| profiles | `.majordomus/profiles/` |
| active task | `.majordomus/state/current.yaml` |
| decisions, open questions | `.majordomus/state/decisions.md`, `open-questions.md` |
| handovers, checkpoints | `.majordomus/state/handovers/`, `state/checkpoints/` |
| history | `.majordomus/state/ledger.jsonl` — read it with `majordomus history` |
| prompt assets | `.majordomus/prompts/` |

This file is generated from the policy. Edit the policy; run `majordomus update`.
