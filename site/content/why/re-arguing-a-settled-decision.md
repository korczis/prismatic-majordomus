+++
title = "Re-arguing a decision that was settled last week"
description = "Why a decision that lives only in a transcript is reopened by the next session, and how a recorded reason and an explicit open question replace the argument."
weight = 6
[extra]
hook = "watched a session undo last week's decision, for the reason it was made"
responsibilities = ["state", "finish"]
commands = ["decision", "question"]
claims = ["decision-record", "decision-attribution", "open-question-gate", "blocker-survives-handover", "blocker-store"]
+++
{% raw %}
## The moment

Last Tuesday the team agreed to normalise the callback URI before comparing it, and ruled
out relaxing the comparison because that would accept forged states. Today a fresh session,
looking at the same mismatch, relaxes the comparison. It is a reasonable fix. It is the one
that was ruled out.

## Why it happens

The decision was made in a conversation, and the reason for it was made there too. The next
session has neither. What it has is the code, which looks like an unexplained constraint,
and a strong prior towards the obvious change. A decision without a reason on record cannot
be reviewed, only re-argued — and the session re-argues it from scratch, with less
information than the first one had. The question that should have stopped the merge ("does
the legacy mobile callback still need the old form?") was a sentence in a handover note that
nobody was obliged to read.

## What Majordomus does

`majordomus decision add` records what was decided and requires `--why`; a decision without
a reason is refused, because the reason is the part the next session needs. `--rejected`
records the alternative that was ruled out and `--evidence` where to look. The task id and
the git head are computed, never typed. An entry is never edited or deleted: `--supersedes`
records that a later decision replaced an earlier one, and refuses text that matches no
recorded decision, so a supersession always points at something real. The `deep-work`
profile makes a decision record a condition of `finish`.

A question that must be answered before the work can be accepted is not a sentence in a
note. `majordomus question add` opens it as a line the tool can read; `finish --outcome
completed` refuses while any unresolved question names the task, and an entry the gate
cannot parse is a failure, not a pass. The question keeps blocking after the work is handed
over to a new task, and `majordomus context` prints every open question above the authored
records, because an open question changes what may be accepted.

## What it does not do

It does not decide anything and it does not judge a reason; a poor reason on record is still
on record, which is what makes it reviewable. It does not stop a worker from making a change
that contradicts a decision — it puts the decision, its reason and its rejected alternative
in front of the worker before the work starts, and lets them be read back with
`decision list` and `search`.

## Try it

```bash
majordomus decision add "Normalise the callback URI before comparing state" \
  --why "the mismatch is a trailing slash, not a forged state parameter" \
  --rejected "relaxing the comparison, which would accept forged states"
majordomus question add "Does the legacy mobile callback still require the old URI form?"
majordomus finish --outcome completed --verify-command "make test"   # FAIL blockers …
```
{% endraw %}
