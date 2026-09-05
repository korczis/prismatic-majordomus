+++
title = "Three roadmaps, and none of them true"
description = "Why a plan kept in prose disagrees with itself the first time anything changes, and how a dependency graph with derived status replaces it."
weight = 10
[extra]
hook = "read three roadmaps for one project and believed none of them"
responsibilities = ["plan"]
commands = ["plan"]
claims = ["project-schema", "project-status-derived", "dag-validation", "execution-waves", "evidence-gates-done", "roadmap-derived", "github-projection"]
+++
{% raw %}
## The moment

The README has a roadmap. GitHub has milestones. A planning document has a diagram. They
were the same plan in June. Today an issue is closed on GitHub, open in the document and
absent from the README, and a worker has just started the piece of work that depends on it.

## Why it happens

A plan written in prose is a snapshot, and every copy of it is a second snapshot that drifts
on its own schedule. Status is the worst of it: "done" typed into a field is an assertion,
not an observation, and it stays true in that field after the world has moved on. Work is
then picked from memory rather than from a dependency order, so something starts before the
thing it needs, and the diagrams are redrawn by hand until someone stops redrawing them.

## What Majordomus does

The plan is two kinds of canonical file, checked against an allowlist where a key nobody
reads is an error. A milestone is an executable specification of an outcome. An issue is a
bounded execution contract: the paths it may touch, the issues it depends on, its acceptance
criteria and the evidence its completion requires. No status is stored anywhere. `READY`,
`BLOCKED`, `ACTIVE`, `VERIFY` and `DONE` are derived from what an issue records about itself
and from the state of its dependencies, every time `majordomus plan` runs; a written status
field is an unknown key.

`plan validate` refuses a cycle, a self-dependency and a dependency on an issue that does
not exist, each by name. `plan waves` computes the execution order from the graph and
reports issues in one wave that touch the same paths as serialised. `plan start` refuses an
issue that is not ready, naming what it waits on; `plan evidence` refuses narrative — it
needs a command or an artifact; `plan done` refuses while any declared evidence is
uncovered or a dependency is not done. The roadmap on this site and the milestones on GitHub
are projections of the same files, and a hand-edited generated region on GitHub is reported
rather than overwritten.

## What it does not do

It does not estimate, schedule or prioritise; it orders by dependency and reports what is
ready. The model is opt-in: a repository without one is skipped by `doctor`, not failed. The
projection runs one way — GitHub is written from the files, and nothing written on GitHub
is ever copied back into them.

## Try it

```bash
majordomus plan validate
majordomus plan next        # the one issue a worker should take now
majordomus plan waves
majordomus plan done I0042  # refused while evidence is missing or a dependency is not done
```
{% endraw %}
