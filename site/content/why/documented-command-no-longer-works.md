+++
title = "The documented command that no longer works"
description = "Examples are written once, in prose, and nothing ever executes them again."
weight = 260
[extra]
id = "documented-command-no-longer-works"
status = "stable"
source = ".ai/repo/why/moments/documented-command-no-longer-works.md"
+++
{% raw %}

## The moment

Somebody copies the command from the getting-started page. It exits 2 with a usage error on
an option that was renamed in the spring. The page has not been wrong for long enough for
anyone to complain, and it has been wrong for everyone who tried it.

## Why it happens

An example in a document is a string. Nothing runs it, so nothing can notice when it stops
working. The command it describes lives in code that changes for good reasons, and the
string does not participate in those changes.

## Why a better model does not fix it

A worker reading the documentation reproduces the broken invocation faithfully, then
diagnoses the failure it caused. The documentation was the input, and it was wrong; no
amount of capability recovers from a wrong specification without paying for the detour.

## What it costs

The most expensive minute in a project: the first one, for someone deciding whether this is
maintained. After that, a steady drip of support questions whose answer is "the docs are
out of date".

## What Majordomus does

The command line is declared once, and the examples are declared beside it as argument
vectors with what each must show — an exit code, a fragment of output, a JSON pointer that
must resolve. The crate's own tests execute every one of them against the built binary in a
disposable repository. The reference document, the machine-readable command model and the
pages on the site are renderings of that same declaration, so a rename reaches all of them
at once. A command that can be run and carries no executed example does not pass validation.

## Before and after

```text
before   docs: majordomus check --explain-all      (renamed in April)

after    every example is argv + Expect, executed by the suite:
         $ cargo test --test cli_examples
         running 27 examples against the built binary ... ok
```

## What it does not do

It does not check prose, and it cannot tell whether an example is a useful one. It makes an
example that no longer works a test failure rather than a reader's discovery.
{% endraw %}
