+++
title = "A blocking question keeps blocking after the work is handed to a new task"
description = "An unresolved entry in state/open-questions.md refuses finish --outcome completed — any entry, not only one the active task opened. Hand the work over, start a new task, and the question still refuses completion, because it is still unanswered."
weight = 82
[extra]
claim_id = "blocker-survives-handover"
status = "guaranteed"
source = "docs/claims/blocker-survives-handover.md"
+++
{% raw %}

## What it means

An unresolved entry in `state/open-questions.md` refuses `finish --outcome completed` — any entry, not only one the active task opened. Hand the work over, start a new task, and the question still refuses completion, because it is still unanswered.

Only completion is refused. `blocked`, `partial`, `no_match` and `failed` are honest statements that the work did not finish, and none of them is gated: refusing them would buy a green gate by forcing somebody to mislabel an outcome.

## How it works

`mj_question_unresolved_any` returns every unresolved line in the store, skipping the HTML comment block that carries the file's own example — every fresh install ships one, and a scan that did not skip it would refuse the first completed finish anybody attempted. `mj_validate_blockers` calls it, names how many are open and quotes the first, and `question resolve` searches the same list, so any task can clear any question. A gate nobody can clear is a gate that gets worked around.

Nothing new is stored. The store is checkout-local, so the questions a gate can see are the ones asked in this working copy, and the entry keeps the id of the task that asked — provenance survives the widening.

## How to see it

```bash
majordomus question add "does the legacy client still need the plain method?"
majordomus finish --outcome blocked                            # allowed; the question is the reason
majordomus start "continue the same work" --scope lib/auth
majordomus finish --outcome completed --verify-command true    # refused: 1 unresolved question(s) on this branch
majordomus question list                                       # it is named, with a number
majordomus question resolve 1 --answer "no; retired in 4.0"
majordomus finish --outcome completed --verify-command true    # accepted
```

## What it does not cover

**A question about abandoned work blocks everything after it.** If the work an open question was about is dropped and nobody answers or retires the question, every later task on that branch is refused completion until somebody writes an answer. That is the case this behaviour is deliberately wrong in. It is loud — `question list` names it and one command clears it — which is why it was chosen over the alternatives, whose failure was silent.

It is scoped to a checkout, not to a repository or a branch: the store lives under `.ai/local/`, which git ignores, so a question asked in another worktree is invisible here until a handover carries it over. Two workers sharing one working copy share its blockers, which is the same coupling they already have on every other local record.

There is no withdrawal verb. A question that turns out not to need answering is resolved with an answer that says so; adding a second way to close one would be a second vocabulary for the same act.

## Why it exists

The gate is the only mechanism in the tool that stops work for a human reason, and it had an escape: the thing that was supposed to refuse acceptance stopped refusing the moment the work moved to a new task. It was found by running the documented end-to-end sequence, not by review, and published as a planned claim with no implementation and no test until `M001` decided between the alternatives.

Two were rejected. **Transferring the question with the handover** needs either mutation of the store, which destroys the record of who asked, or a lineage field, which is new state; and it blocks unrelated work whenever a handover passes to something else. **Gating on scope overlap** between the asking task and the finishing one is a heuristic standing in for topical relevance, and it fails open when an archived task record is missing — the same class of defect being repaired. Both fail silently. This one fails loudly, and that was the deciding property.
{% endraw %}
