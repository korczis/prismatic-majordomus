+++
title = "Say what reached the remote and what did not"
description = "The obligations that reach past the working tree are settled by asking git and the published site, so a report cannot call uncommitted work committed or unpushed work pushed."
weight = 19
[extra]
id = "say-what-reached-the-remote-and-what-did-not"
source = ".ai/repo/use-cases/say-what-reached-the-remote-and-what-did-not.md"
category = "completion"
maturity = "described"
+++

## Situation

Implemented, committed, pushed, integrated, published and deployed are six different facts,
and a report that treats them as one is not a report. A worker can finish a change, run its
tests, satisfy every line of the finish contract, and leave the work sitting in the working
tree of one laptop.

The obligation vocabulary already had tokens for all six. What it did not have was anybody
asking: each was discharged by a person running something and typing that they had. Git
holds four of those answers and has held them all along.

## Scenario

```yaml
setup: task-owes-outer
given:
  - a repository with the layer installed, an active task that owes commit and push, and the work still in the working tree
steps:
  - id: recording-it-changes-nothing
    run: ['evidence', '--covers', 'commit', '--command', 'git commit']
    expect:
      exit: 0
      output: 'evidence: commit recorded'
  - id: the-tree-is-asked-instead
    run: ['finish', '--outcome', 'completed', '--note', 'done']
    expect:
      exit: 10
      output: 'still in the working tree'
  - id: what-cannot-be-settled-says-so
    run: ['check']
    expect:
      exit: 0
      output: 'the checkout has no remote'
then:
  - a token the tool can settle is settled live at HEAD, and a recorded line neither discharges it nor rescues it
  - a checkout that cannot settle a token falls back to the recorded evidence and says what it could not establish
  - unreachable and unpublished are different findings, and neither is reported as the other
```

## Outcome

The words in the report are the words for what happened. Work in the tree is not committed,
a commit the remote has not seen is not pushed, a branch the trunk does not reach is not
integrated, and a site that never answered is not a site serving an old commit. None of
those distinctions costs a worker anything to maintain, because none of them is maintained
by a worker.
