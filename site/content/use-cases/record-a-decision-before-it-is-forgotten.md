+++
title = "Record a decision as data, and prove the tool cannot accept it for you"
description = "Read the decisions a repository holds, validate the whole set in one command, and watch the tool refuse to write the one status only a person may write."
weight = 37
[extra]
id = "record-a-decision-before-it-is-forgotten"
source = ".ai/repo/use-cases/record-a-decision-before-it-is-forgotten.md"
category = "knowledge"
maturity = "described"
+++

## Situation

The decisions a repository lives by were made in conversations that are gone by the next morning, and the only record is a paragraph in a chat log nobody can find. The reasoning is lost first, so six months later the code looks arbitrary and somebody quietly undoes it.

## Scenario

```yaml
setup: adr-related
given:
  - 'Majordomus installed, with one decision proposed and committed'
  - 'discovery is over the tracked tree, so a decision git does not hold is not yet part of the layer'
steps:
  - id: what-was-decided
    run: ['adr', 'list']
    note: 'one line per decision: identity, status, date, title'
    expect:
      exit: 0
      stdout_contains: ['adr-0001', 'proposed']
  - id: the-record-itself
    run: ['adr', 'show', 'adr-0001']
    note: 'the path, then the file as written: the provenance says the tool derived it'
    expect:
      exit: 0
      stdout_contains: ['schema: adr/v1', 'origin: extracted', 'file:docs/d']
  - id: the-set-is-sound
    run: ['adr', 'check']
    note: 'identities, statuses, supersession and every reference, in one pass'
    expect:
      exit: 0
      stdout_contains: ['every identity unique']
  - id: what-it-put-in-force
    run: ['adr', 'show', 'adr-0001']
    note: 'related is the forward edge: the rule the decision declares, the file it reaches'
    expect:
      exit: 0
      stdout_contains: ['related:', 'rule:majordomus.adr-integrity']
  - id: the-graph-reads-it-backwards
    run: ['knowledge', 'edges', '--type', 'declares']
    note: 'the same fact as an edge, with the front-matter key that stated it as provenance; nothing wrote the reverse direction down'
    expect:
      exit: 0
      stdout_contains: ['adr:adr-0001', 'rule:majordomus.adr-integrity', 'related.0']
  - id: what-a-change-reaches
    run: ['adr', 'affected']
    note: 'the same edges read from a change set: which decisions this work touches. A clean tree reaches nothing, and the exit code never says a decision stopped holding'
    expect:
      exit: 0
      stdout_contains: ['no decision names anything this change set touches']
  - id: not-yours-to-choose
    run: ['adr', 'propose', 'A decision that accepts itself', '--status', 'accepted']
    note: 'the refusal that matters: a tool that can write accepted turns its inference into repository truth'
    expect:
      exit: 15
      stdout_contains: ['not yours to choose']
  - id: evidence-must-resolve
    run: ['adr', 'propose', 'A decision derived from a file nobody has', '--from', 'file:docs/nosuch']
    note: 'an extracted record without evidence is an assertion, and is refused as one'
    expect:
      exit: 2
      stdout_contains: ['names a path that does not exist']
then:
  - 'a reference a decision makes is validated where its type says the target lives'
  - 'the reverse direction is the graph read backwards, never a second list'
  - 'a change set names the decisions it reaches; whether they still hold is a person to read, not an exit code'
  - 'a decision is one validated file under .ai/repo/adrs/, discovered as data'
  - 'no invocation of the tool writes status accepted'
```

## Outcome

The decision is a file with an identity nothing else claims, the evidence it came from, and a status that says plainly whether a person has accepted it. The tool proposes; only a person accepts, and the command line has no way to say otherwise.
