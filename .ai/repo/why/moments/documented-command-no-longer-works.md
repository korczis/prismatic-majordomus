---
schema: moment/v1
id: documented-command-no-longer-works
kind: moment
title: 'The documented command that no longer works'
short_title: 'Broken example'
hook: 'pasted a command from the documentation and watched it fail'
summary: 'Examples are written once, in prose, and nothing ever executes them again.'
status: stable
severity: medium
frequency: common
weight: 260
audiences: [open-source-maintainer, platform-team, agency, ai-native-team]
areas: [documentation, verification]
lifecycle: [onboarding, maintenance]
tags: [documentation, examples, drift, onboarding]
signals:
  - id: example-fails
    text: 'A command copied from the documentation failed this month.'
  - id: examples-never-run
    text: 'The examples in the documentation are not executed by anything.'
  - id: flag-renamed
    text: 'An option was renamed and the places that show it were updated by hand, or not at all.'
examples:
  - id: first-command
    audience: agency
    title: 'The first command a newcomer runs'
    before: 'The quick-start command fails on an option that was renamed a release ago; the newcomer''s first impression is that nothing here is maintained.'
    after: 'Every documented example is an argument vector the test suite executes against the built binary, with what it must print.'
  - id: renamed-flag
    audience: platform-team
    title: 'A rename that reached five documents'
    before: 'An option is renamed; five documents show the old one and are found one complaint at a time.'
    after: 'The reference is rendered from the declaration and the examples beside it; there is no second copy to miss.'
  - id: agent-copies-it
    audience: ai-native-team
    title: 'A worker following the documentation'
    before: 'A worker builds a script around a documented invocation that has not worked for months.'
    after: 'A command with no executed example does not pass the crate''s own gate, so the documentation cannot get ahead of the binary.'
commands: [doctor, usecase, bench]
capabilities: [capabilities.list]
responsibilities: [doctor, projection]
claims: [executable-reference-derived, command-surface, command-coverage, use-case-evidence, reproduce-command]
doctrines: [project.native-cli-documented, project.use-case-evidence, majordomus.command-surface, majordomus.command-coverage]
use_cases: [add-a-use-case-and-prove-it, know-which-tool-is-running, read-the-rules-the-tool-applies]
related: [feature-without-a-test, api-changed-contract-did-not, first-hour-in-an-unfamiliar-repository]
aliases: ['broken docs example', 'stale CLI documentation', 'copy-paste fails']
---

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
