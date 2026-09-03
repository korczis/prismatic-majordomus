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
   ... work ...
majordomus check              # before claiming anything is done
majordomus handover < note.md # to continue in another session, or
majordomus finish --outcome <completed|partial|blocked|no_match|failed> --verify-command "<cmd>"
```

Checkpoint by running `majordomus check --checkpoint` at least every
{{CHECKPOINT_DEFAULT}}. Default profile: `{{DEFAULT_PROFILE}}`.

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
{{REQUIRED_SECTIONS}}. Front matter is computed; do not write it.

### Where things are

| | |
|---|---|
| canonical policy | `.majordomus/policy.yaml` |
| profiles | `.majordomus/profiles/` |
| active task | `.majordomus/state/current.yaml` |
| decisions, open questions | `.majordomus/state/decisions.md`, `open-questions.md` |
| handovers | `.majordomus/state/handovers/` |

This file is generated from the policy. Edit the policy; run `majordomus update`.
