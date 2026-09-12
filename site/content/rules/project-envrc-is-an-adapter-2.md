+++
title = "The shell entry point is an adapter, and it makes one call"
description = "The shell entry point is an adapter, and it makes one call"
weight = 81
[extra]
kind = "rule"
slug = "project-envrc-is-an-adapter-2"
identity = "project.envrc-is-an-adapter@2"
status = "active"
source = ".ai/repo/rules/project/envrc-is-an-adapter.v2.md"
+++
{% raw %}

## What changed in version 2

Version 1 listed what the entry point may do, and the list had a shape nobody had named:
every item on it was a *call to the tool*, and the only thing that made them four items
rather than one was that the tool had four small commands and no whole one. ADR 0043 gave
it the whole one — `majordomus env enter`, which exports, draws the banner, refreshes the
workflow bridge and ensures the runtime — and with it the list collapses to its real
content: **one call, to the bootstrap command**, and nothing else but putting a directory
on the path and declaring a file to watch.

That is a narrowing, not a loosening. Everything version 1 forbade is still forbidden, word
for word. What version 2 adds is that a *second* call to the tool is now a finding too —
because two calls are two readings of the repository on the hot path of every `cd`, and
because the moment the entry file chooses which of several commands to run, it has started
deciding things, which is the failure this rule is entirely about.

What that one call now *does* changed at the same time, and that half belongs to
`project.entry-converges@2`: entering the repository brings its runtime up. This rule says
nothing about that. It says the entry file does not do it — the executable does.

## Rationale

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

## Required behaviour

A file a shell evaluates on entering the repository — `.envrc`, or whatever an equivalent
tool reads — may:

- put a directory on the path;
- declare a file to watch, so the tool is asked again when what it reported changes;
- make **exactly one** call to the tool, which is the bootstrap command (`env enter`), and
  `eval` what it printed on standard output;
- source a machine-local file of the person's own, which the repository never sees and
  never reads — a place for their secrets and overrides, last, so that a value there wins;
- assign a variable to a literal, or to a value that one call printed.

It may not:

- call the tool a second time, whatever the second call is for: two calls are two readings
  of the repository on every `cd`, and choosing between commands is a decision, which
  belongs inside the executable where it is typed, benchmarked and tested;
- run a version-control command, or any program that inspects the repository — `git`,
  `grep`, `sed`, `awk`, `find`, `jq`, `wc`, `cat` over a tracked file, `cargo metadata`;
- build anything, or invoke a package manager or a compiler;
- reach the network itself, in any form, including a loopback HTTP request. What the one
  call it makes does with a loopback address a server of this checkout already published is
  the executable's business, and `project.entry-converges` is what bounds it;
- carry a count, a version, a URL, a provider name, a branch or a workflow command as a
  literal, when the tool can report it;
- carry a port, a pid, a timeout, an idle life or any other parameter of the runtime: those
  are the bootstrap command's own defaults, declared once in the command line's declaration
  and changed there.

The adapter it calls resolves the executable without building it: a checkout that has not
built the executable yet prints one line naming the recipe that builds it and exits 0,
because entering a directory must not fail.

Everything the entry point shows is resolved by the tool in its fast resolution, which
never builds the index, never contacts anything but a loopback address a running server
published, and reports what it could not resolve as unknown rather than as a default.

## Failure behaviour

`test/cases/100_environment.sh` reads the entry point and fails, naming the line, when it
carries a forbidden command, calls the tool more than once, or exceeds its complexity
budget. It is a behavioural case, so
the shell suite runs it on every change that selects the suite, and the `layer` and `shell`
gate classes both select it.

There is no runtime enforcement and there should not be: the file is evaluated by a shell
this tool does not control, and a check that ran there would be one more thing running on
every `cd`.

## Verification

`test/cases/100_environment.sh` holds the entry point to the list above, proves that it
makes exactly one call to the tool and that the call is the bootstrap command, proves the
adapter exits 0 and says what to run when the executable is absent, proves the exported
script is assignments only, and proves that the check can fail: a forbidden line planted in
one copy and a second call to the tool planted in another must both be rejected by the same
reader.

`apps/majordomus-cli/tests/environment.rs` proves the half the entry point delegates to:
that the fast resolution builds no index, that a shell evaluates the exported script back
to the values it was given whatever the repository path contains, and that the banner is
silent when nothing is watching.
{% endraw %}
