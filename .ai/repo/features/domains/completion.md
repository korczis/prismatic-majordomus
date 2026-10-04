---
schema: domain/v1
id: completion
kind: domain
title: Completion
headline: "Done is a contract the repository evaluates line by line. Work is not accepted while any line fails, and a refusal stays on the record."
problem: "Done is cheap. A worker can report success before the repository's own checks hold."
status: stable
weight: 50
tags: [completion, finish, verification]
---

# Completion

When a piece of work counts as finished, and who decides: the finish contract a task is
evaluated against, the coverage a change is held to, and the gates CI runs before anything
is published. A worker can always say it is finished; here that sentence is a claim the
repository checks, and nothing is recorded as done while any line of the contract fails.

Not in this domain: the proof behind a public claim, which is evidence; and the scope a
task was allowed, which is coordination.
