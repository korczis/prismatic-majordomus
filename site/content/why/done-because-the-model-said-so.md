+++
title = "Accepting \"done\" because the model said so"
description = "Why a sentence is not a completion criterion, and how a finish contract with typed outcomes replaces it."
weight = 4
[extra]
hook = "accepted \"done\" because the model said so, and paid for it the next morning"
responsibilities = ["finish"]
commands = ["finish"]
claims = ["finish-contract", "typed-outcome", "reproduce-command"]
+++
{% raw %}
## The moment

"Done. All tests pass." You merge. In the morning the pipeline is red, the change touched a
file nobody asked about, and the "tests" that passed were the two the worker chose to run.

## Why it happens

"Done" was a word in a transcript. No one had written down, before the work started, what
would have to be true for it to be accepted — so the worker's own claim was the only
evidence, and a fluent claim is cheap. The environments studied were full of this: status
fields with free-text values, completion notes that were never written, and a documented
per-step audit trail that no gate ever checked.

## What Majordomus does

`majordomus finish` evaluates a contract, line by line, and prints each line as pass or
fail: touched files within the declared scope; the project's own verification command ran
and exited zero, with its command, exit code and duration recorded; the task record still
describes this checkout; no open question for this task is unresolved; a handover or
completion note with the required sections exists. If any line fails, nothing is written.

The outcome is a value from a closed vocabulary — `completed`, `partial`, `blocked`,
`no_match`, `failed` — not prose. `no_match` means the work was done and the thing sought
does not exist; `failed` means the work could not be done. They look alike in a chat and are
different facts. Every refusal names the command that reproduces the failing line.

## What it does not do

It runs the verification command you give it; it does not decide which tests matter. The
regression-test requirement in the `debugging` profile is a path heuristic and says so in
its message. It does not review code.

## Try it

```bash
majordomus finish --outcome completed --verify-command "make test"
# refused? every failing line names what is missing and how to reproduce it
```
{% endraw %}
