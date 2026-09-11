---
id: project.envrc-is-an-adapter
version: 1
kind: rule
title: The shell entry point is an adapter, and repository facts come from one typed source
description: A file a shell evaluates on entering the repository resolves the tool, evaluates what the tool exports and asks it to render; it reads nothing about the repository itself, builds nothing, and reaches no network, so that entering a directory cannot be slow, cannot fail, and cannot become a second implementation of what the executable already knows.
statement: A file a shell evaluates on entering the repository resolves the tool, evaluates what the tool exports and asks the tool to render; it does not itself read the repository, build anything, or reach the network.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.no-network-no-eval@1, project.hot-path-reads-once@1]
tags: [environment, shell, performance]

x-majordomus:
  tests: [test/cases/100_environment.sh]
---

# Rationale

A `.envrc` is the most tempting place in a repository to put a shell pipeline, and the
worst. It runs on every entry into the directory, on every machine, in whatever shell
somebody happens to use; nothing tests it; nobody reviews it; and every fact it computes
is a fact something else in the repository already knows how to compute properly.

The failure is not hypothetical, it is the normal end state. A `git rev-parse` to show the
branch. A `grep` over `Cargo.toml` for the version, which then disagrees with the crate.
A `find | wc -l` to count the rules, which counts files that do not parse and reports a
number no other surface would report. A `curl` to see whether the server is up, which
hangs for thirty seconds when it is not. Each is added by somebody reasonable, one line at
a time, and the result is a shell program on the hot path of every `cd` that no gate
covers and that says things the tool would contradict.

The same facts already have one canonical typed source — `environment::RepositoryEnvironment`
— which the command line, the HTTP route, the MCP resource, the Cockpit and the
documentation all render. The shell entry point is one more consumer of it, and the only
thing it may do that the others do not is evaluate assignments, because that is the one
thing a process cannot do to its parent shell.

The cost matters as much as the correctness. Entering a repository is something a person
does dozens of times a day and never chose to wait for; and a build started there is a
build started at the worst possible moment.

# Required behaviour

A file a shell evaluates on entering the repository — `.envrc`, or whatever an equivalent
tool reads — may:

- put a directory on the path;
- declare a file to watch, so the tool is asked again when what it reported changes;
- `eval` the assignments the tool prints;
- run the tool to render a banner;
- assign a variable to a literal, or to a value the tool printed.

It may not:

- run a version-control command, or any program that inspects the repository — `git`,
  `grep`, `sed`, `awk`, `find`, `jq`, `wc`, `cat` over a tracked file, `cargo metadata`;
- build anything, or invoke a package manager or a compiler;
- reach the network, in any form, including a loopback HTTP request;
- carry a count, a version, a URL, a provider name, a branch or a workflow command as a
  literal, when the tool can report it.

The adapter it calls resolves the executable without building it: a checkout that has not
built the executable yet prints one line naming the recipe that builds it and exits 0,
because entering a directory must not fail.

Everything the entry point shows is resolved by the tool in its fast resolution, which
never builds the index, never contacts anything but a loopback address a running server
published, and reports what it could not resolve as unknown rather than as a default.

# Failure behaviour

`test/cases/100_environment.sh` reads the entry point and fails, naming the line, when it
carries a forbidden command or exceeds its complexity budget. It is a behavioural case, so
the shell suite runs it on every change that selects the suite, and the `layer` and `shell`
gate classes both select it.

There is no runtime enforcement and there should not be: the file is evaluated by a shell
this tool does not control, and a check that ran there would be one more thing running on
every `cd`.

# Verification

`test/cases/100_environment.sh` holds the entry point to the list above, proves the
adapter exits 0 and says what to run when the executable is absent, proves the exported
script is assignments only, and proves that the check can fail by introducing a forbidden
line into a copy and requiring the same check to reject it.

`apps/majordomus-cli/tests/environment.rs` proves the half the entry point delegates to:
that the fast resolution builds no index, that a shell evaluates the exported script back
to the values it was given whatever the repository path contains, and that the banner is
silent when nothing is watching.
