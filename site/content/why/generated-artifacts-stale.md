+++
title = "Green tests over stale generated files"
description = "Tests exercise the code and say nothing about the committed outputs derived from it, so the build is green and the artifacts are wrong."
weight = 270
[extra]
id = "generated-artifacts-stale"
status = "stable"
source = ".ai/repo/why/moments/generated-artifacts-stale.md"
+++
{% raw %}

## The moment

Everything is green. The tests pass, the linter is quiet, and four committed files that are
generated from the code describe the code as it was before the last change. Nothing in the
pipeline looks at them, because they are not code and they are not tests.

## Why it happens

Generated files that are committed occupy an awkward category: they are reviewed like
source and produced like output. Regenerating them is a step, and a step that is not run by
the same thing that runs the tests will be skipped by whoever is in a hurry — which,
increasingly, is a worker that was asked to change the source and did exactly that.

## Why a better model does not fix it

Regeneration is a build step, not a judgement. A worker that did not run it did not fail to
reason; it was never told, and telling it every time is the manual synchronisation the
generation was supposed to remove.

## What it costs

Reviewers read a stale artifact and approve it. Consumers of the artifact — a site, a
client generator, another tool — serve last month's truth. And the tree is no longer
reproducible: running the generator produces a diff, which everyone learns to ignore.

## What Majordomus does

The derivation graph is written down once, in dependency order, so a person and CI run the
same thing in the same sequence. Every committed derived artifact has a check that
regenerates it and compares: `generate --check` names each file that differs or is missing
and exits non-zero, and the site's data has the same check against the hash of its inputs.
Running the whole derivation twice changes nothing, which is what the check compares
against.

## Before and after

```text
before   tests: green      docs/generated/*.json: from two commits ago

after    $ scripts/derive-check
         stale: docs/generated/openapi.json (differs)
                site/data/registry/registry.json (differs)
         exit 10
```

## What it does not do

It does not decide what should be generated, and it will not regenerate silently during a
check — a check that writes is not a check. It names what is stale and the command that
fixes it.
{% endraw %}
