---
schema: context/v1
id: ai.repo.post-mortems
kind: context
title: Post-mortems
description: One record per cause that turned the trunk's CI red — the runs it failed, why, the components the cause lives in, what fixed it and what stops it returning; `majordomus postmortem` reads them and searches a new failure for their signals.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/post-mortem/post-mortem.v1.proto, share/kinds.yaml, docs/POST_MORTEMS.md, .ai/repo/rules/project/a-ci-failure-has-a-post-mortem.v1.md]
---

# Post-mortems

A file here answers one or more failed runs of a workflow on the trunk. Its front matter is
the part a program reads: `runs` names the runs it answers, `cause` gives the class and one
sentence, `components` says where the cause lives, `signals` is the text a failing log
shows, `fixed_by` and `prevention` say what removed it and what guards against it, and
`related` names the issues, decisions, intents or claims it concerns. `components`,
`prevention` and `related` are references of the layer (`file:`, `test:`, `rule:`,
`issue:`, ...), so each is a link in both directions. Its body
has four sections a person reads when the same job is red again: What failed, Root cause,
Fix, Prevention.

Write one when the trunk goes red, in the pull request that fixes it or in one of its own:

- `majordomus postmortem owed` lists the failed runs no record names yet;
- `majordomus postmortem draft --run <id>` prints a schema-valid draft for one of them;
- `majordomus postmortem check` judges every record here;
- `majordomus postmortem diagnose` searches a failure's text for the signals of every
  record and names the ones that match.

A record starts as `draft` with the cause `undetermined`, becomes `analysed` when the cause
is named with its components and signals, and `resolved` when `fixed_by` names what is on
the trunk. A sentence that was not verified says so; "not established" is a valid finding
and a guess is not. The file name is the record's `id` with `.md`.

The schema is `post-mortem/v1`
(`share/schemas/majordomus/post-mortem/post-mortem.v1.proto`), validated on every index
build. The rule is `project.a-ci-failure-has-a-post-mortem`, the decision ADR 0156, and
`docs/POST_MORTEMS.md` the reference.
