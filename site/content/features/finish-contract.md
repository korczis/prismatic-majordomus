+++
title = "Done is a contract, evaluated line by line and refused when unmet"
description = "A task starts with a declared scope and a profile; check reports whether the task is consistent with policy, scope and state; finish evaluates the policy's contract — scope respected, verification ran, state updated, no open blockers, a note present — and refuses with the reproducing command when any line fails."
weight = 80
[extra]
id = "finish-contract"
status = "stable"
source = ".ai/repo/features/finish-contract.md"
+++
{% raw %}

## What it does

The outcome of a task is a typed field with a closed vocabulary, never a sentence.
`majordomus finish --outcome completed` runs the verification command the profile requires,
walks every line the policy selects, prints each as pass or fail with the command that
reproduces a failure, and writes nothing when any line fails; the task stays open. An open
question that names the task blocks acceptance until it is resolved, and a blocker survives
a handover rather than being lost with the conversation.

`no_match` and `failed` are different facts: the thing sought does not exist, or the work
could not be done. A supervisor that cannot tell them apart cannot decide whether to retry,
escalate or accept, so the field decides and prose never does.

## What it does not do

It does not prevent a worker from touching a file outside its scope while it works; it
detects the file at check and at finish and refuses to accept the work. The regression-test
line is a path heuristic and says so. Nothing here measures tokens or cost.
{% endraw %}
