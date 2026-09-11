+++
title = "Add a use case and let the tool prove it"
description = "Scaffold a draft for an uncovered command, validate it, run its scenario against the real tool, and see the coverage move."
weight = 33
[extra]
id = "add-a-use-case-and-prove-it"
source = ".ai/repo/use-cases/add-a-use-case-and-prove-it.md"
category = "extension"
maturity = "described"
+++

## Situation

A command gained a behaviour and nobody wrote down what a person does with it. The documentation will drift the day it is written by hand, and a paragraph proves nothing.

## Scenario

```yaml
setup: installed
given:
  - 'Majordomus installed; the skeleton seeds the use-case section with no use case yet'
steps:
  - id: nothing-yet
    run: ['usecase', 'list']
    note: 'the section exists and is empty'
    expect:
      exit: 0
      stdout_contains: ['use cases: 0 in']
  - id: the-gaps
    run: ['usecase', 'coverage']
    note: 'every public command is a target; a fresh policy reports gaps and fails on none'
    expect:
      exit: 0
      stdout_contains: ['^command +doctor +0 +0 +0 +gap +advisory']
  - id: scaffold
    run: ['usecase', 'scaffold', '--for', 'command:doctor']
    note: 'a draft from what the registry, the fixture and the claims already know; nothing in it is verified'
    expect:
      exit: 0
      stdout_contains: ['wrote: \.ai/repo/use-cases/doctor-draft\.md']
  - id: validate
    run: ['usecase', 'validate']
    note: 'the draft resolves: its command, setup and category exist'
    expect:
      exit: 0
      stdout_contains: ['usecase validate: 0 failure']
  - id: run-it
    run: ['usecase', 'run', 'doctor-draft']
    note: 'the scaffolded scenario executes the real command in a disposable repository'
    expect:
      exit: 0
      stdout_contains: ['doctor-draft +pass']
  - id: still-a-gap
    run: ['usecase', 'coverage']
    note: 'a draft never counts; coverage moves when the use case is active'
    expect:
      exit: 0
      stdout_contains: ['^command +doctor +0 +0 +0 +gap']
then:
  - 'a use case is a file; everything else (the page, the links, the tally) follows'
  - 'a draft can run but cannot cover: status is authored, maturity is observed'
```

## Outcome

`usecase scaffold` writes a draft with the facts the tool already has, `usecase run` executes its scenario against the real tool and records the evidence, and `usecase coverage` says what is still missing. The draft counts for nothing until it is made active.
