+++
title = "A capability the documentation promises and nothing proves"
description = "A sentence describing what the software does is written once and never connected to anything that would fail if it stopped being true."
weight = 240
[extra]
id = "feature-without-a-test"
status = "stable"
source = ".ai/repo/why/moments/feature-without-a-test.md"
+++
{% raw %}

## The moment

The README says the tool validates the input before writing. It used to. The validation was
removed during a refactor eight months ago and the sentence stayed, because nothing related
the sentence to the code.

## Why it happens

Prose and behaviour are maintained by different acts. Deleting code is a change with a
diff; deleting the sentence that described it is an act of remembering. Nobody remembers,
and no check exists, because the sentence is not a testable artefact — it is a paragraph.

## Why a better model does not fix it

A worker asked to implement against the documentation implements against a description of a
system that no longer exists, faithfully. Documentation that cannot be false is
indistinguishable from documentation that is true.

## What it costs

Users and contributors build on a promise that is not kept. Worse, the promise is used as a
specification: somebody restores the described behaviour badly, or builds a layer on top of
a guarantee that was never there.

## What Majordomus does

A capability sentence is a claim with a status — guaranteed, advisory, planned or rejected —
the file that implements it and the behavioural case that proves it. A guaranteed claim
without a real implementation and a real test fails the check, and a sentence that cannot be
backed is phrased as a target instead. Use cases go further: each names the commands and
claims it exercises and carries a scenario that is executed against the real tool, so the
example on a page is the output of a run rather than a paste.

## Before and after

```text
before   README: "validates input before writing"     (last true in January)

after    $ majordomus doctor
         FAIL claims  input-validation — status guaranteed, test '-' 
                      [reproduce: majordomus usecase coverage]
```

## What it does not do

It does not write tests, and it cannot tell whether a test is a good one. It refuses the
combination of a strong claim and no evidence, which is the state in which documentation
starts lying.
{% endraw %}
