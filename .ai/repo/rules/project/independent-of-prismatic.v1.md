---
id: project.independent-of-prismatic
version: 1
kind: rule
title: Independent of Prismatic
description: Majordomus builds, tests, runs and deploys with no Prismatic present; a useful Prismatic idea is ported and owned here, never linked.
statement: Majordomus never depends on Prismatic — no import, package, repository, submodule, shared storage, internal API, running service or deployment topology — and anything taken from it is ported into a Majordomus-native design with its own interface, tests and documentation.
status: active
class: blocking
depends_on: [project.clean-room@1]
tags: [provenance, dependencies]

x-majordomus:
  tests: [scripts/ci/prismatic-independence-check, test/cases/332_independent_of_prismatic.sh]
---

# Rationale

Prismatic is prior art, not infrastructure. `project.clean-room` keeps its vocabulary out of
what is written here; this rule keeps its code, services and topology out of what runs. A tool
that supervises other repositories cannot need a particular one of them to build, start or
deploy, and a dependency that exists only on the author's machine is discovered by the first
person who does not have it.

# Required behaviour

Majordomus never depends on Prismatic at compile time, at run time, at deploy time or
operationally. None of these is allowed:

- an import from a Prismatic module or application,
- a package, repository or path dependency on a Prismatic project,
- a git submodule pointing at one,
- storage or runtime state shared with one,
- a call into an internal or undocumented Prismatic interface,
- a requirement that a Prismatic service be running,
- a deployment topology that assumes Prismatic exists.

A Prismatic idea may still be useful here — a pattern, an algorithm, a state model, a
lesson. It enters by porting, in this order:

1. name the Majordomus requirement it answers,
2. name the invariant the original was preserving,
3. separate that invariant from the assumptions of the environment it came from,
4. decide whether it belongs in Majordomus at all,
5. design a Majordomus-native interface for it,
6. port or reimplement the minimum it needs,
7. prove it with tests in this repository,
8. document it in this repository,
9. confirm no link back remains,
10. own the result like any other code here.

A design document may record the provenance — "inspired by the Prismatic session model,
implemented independently" — because provenance is history, not a dependency.

# Failure behaviour

A change that introduces a dependency is refused. `scripts/ci/prismatic-independence-check`
exits 10 and names the file and line; the `prismatic-independence` gate runs it on every CI
plan.

# Verification

`scripts/ci/prismatic-independence-check` decides the part of this rule a tree can show: no
`.gitmodules` entry and no gitlink naming a Prismatic project, and no reference to one in the
surfaces that build, run or deploy the tool — `bin/`, `lib/`, `share/`, `apps/`, `scripts/`,
the hooks, the workflows, the dependency manifests and lock files, and the client and
deployment configuration. The project's own name, Prismatic Majordomus in any of its
spellings, is not a reference. `test/cases/332_independent_of_prismatic.sh` proves the check
by mutation.

The check is deliberately partial and says so. It cannot see a service reached through a
hostname nothing names, a document that tells a person to start one, or a port that copied
code without redesigning it. The exit test for those is a reader's: someone with no access to
Prismatic can build Majordomus, run its tests, understand its contracts from its own
documents, operate the feature, and change it without consulting Prismatic.
