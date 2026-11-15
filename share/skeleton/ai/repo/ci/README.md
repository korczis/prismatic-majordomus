---
schema: context/v1
id: ai.repo.ci
kind: context
title: Continuous integration
description: The CI model of this repository, gates.yaml - the gates it runs, the job that runs each, and which changed paths select them.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Continuous integration

`gates.yaml` is the model of what CI proves, read by `majordomus check` and `finish` to plan
the gates a task's change selects. A gate is one command whose exit status is its verdict,
run by one CI job; a class names the paths that select it; a lane says which moment needs it
green (`feedback` for a pull request, `proof` for the combined top a landing integrates).

`majordomus init --ci` laid the file down with one gate every supervised repository can run,
`skills-verify`, and a commented example of a test job to adapt. Declare the gates your CI
runs, run each in the job it names, and report its verdict:

```sh
majordomus evidence --gate <id> --exit <status> --command "<the command it ran>"
majordomus evidence --run-gates      # or run every gate the task's change selects, here
```

A gate the change selects that has never reported is a named debt and refuses nothing; one
known to be failing, or whose verdict cannot be read, refuses `finish --outcome completed`.
