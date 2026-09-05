---
id: record-a-decision-before-it-is-forgotten
kind: use-case
title: 'Record a decision as data, and prove the tool cannot accept it for you'
summary: 'Read the decisions a repository holds, validate the whole set in one command, and watch the tool refuse to write the one status only a person may write.'
category: knowledge
status: active
target: guaranteed
actors: [maintainer, agent]
difficulty: basic
commands: [adr]
doctrines: [majordomus.adr-integrity]
claims: [adr-catalogue, adr-propose]
responsibilities: [layer]
applications: [long-running-work]
scenario:
  setup: adr-recorded
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
    - 'a decision is one validated file under .ai/repo/adrs/, discovered as data'
    - 'no invocation of the tool writes status accepted'
---

# Situation

The decisions a repository lives by were made in conversations that are gone by the next morning, and the only record is a paragraph in a chat log nobody can find. The reasoning is lost first, so six months later the code looks arbitrary and somebody quietly undoes it.

# Outcome

The decision is a file with an identity nothing else claims, the evidence it came from, and a status that says plainly whether a person has accepted it. The tool proposes; only a person accepts, and the command line has no way to say otherwise.
