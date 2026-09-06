+++
title = "Re-arguing a decision that was settled last week"
description = "A decision whose reason lived in a conversation cannot be reviewed, only re-argued — by a worker with less information than the first one had."
weight = 60
[extra]
id = "re-arguing-a-settled-decision"
status = "stable"
source = ".ai/repo/why/moments/re-arguing-a-settled-decision.md"
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

## Why a better model does not fix it

The second session's reasoning was sound. Given a strict comparison, a mismatch and no
recorded reason for the strictness, relaxing the comparison is the correct inference. The
missing input was not intelligence; it was the sentence "we rejected this, and here is what
it would let through".

## What it costs

The rework, plus the security regression that the rework reintroduces, plus the argument
when somebody notices. Worst of all, the second decision is usually made with less context
than the first, so the repository trends towards whichever answer is easiest to reach from
the code alone.

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
records.

## Before and after

```text
before   the reason existed in a conversation that ended

after    majordomus decision add "Normalise the callback URI before comparing state" \
           --why "the mismatch is a trailing slash, not a forged state parameter" \
           --rejected "relaxing the comparison, which would accept forged states"
         # the next session is assembled with it, and `search` finds it
```

## How to verify it

Try to record a decision without `--why`: it is refused. Record one with a rejected
alternative, then read it back with `decision list` and `search`; the task id and head were
computed from git, not typed.

## What it does not do

It does not decide anything and it does not judge a reason; a poor reason on record is still
on record, which is what makes it reviewable. It does not stop a worker from making a change
that contradicts a decision — it puts the decision, its reason and its rejected alternative
in front of the worker before the work starts.
{% endraw %}
