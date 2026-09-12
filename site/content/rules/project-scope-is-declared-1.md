+++
title = "What a worker reads is declared once, and nothing outside it is read or served"
description = "What a worker reads is declared once, and nothing outside it is read or served"
weight = 120
[extra]
kind = "rule"
slug = "project-scope-is-declared-1"
identity = "project.scope-is-declared@1"
status = "active"
source = ".ai/repo/rules/project/scope-is-declared.v1.md"
+++
{% raw %}

## Rationale

A worker that reads whatever it finds reads build outputs, vendored code, a megabyte of
fixture, an image, or a secret, and quotes it back. The boundary was implicit: an
exclusion here, a size limit there, a `.gitignore` nobody meant as policy. One declaration
makes the boundary reviewable, the same for every provider and every projection, and
answerable: `majordomus scope <path>` says in or out and names the rule.

## Required behaviour

`.ai/repo/scope.yaml` (the manifest's `scope` section) declares `in`, the pathspecs a
worker reads, and `out`, what it never reads: paths, secrets, generated assets, archives,
images, video, PDF, database dumps, fixtures over a limit, files over a limit, binary
content. `out` wins over `in`; a path matching nothing is out as `undeclared`. A
repository that declares none is read under the distribution's default, the file
`majordomus init` seeds, and the origin is reported, never assumed. The Rust executable
compiles the declaration once at start-up; discovery drops a source outside it with a
diagnostic naming the rule; the index, and so every MCP, HTTP and command-line projection
of it, holds nothing outside the scope; `repository.scope` answers the declaration and
every tracked file tallied against it; `repository.scope_classify` judges any path by
name, then size, then content; a tracked secret is a `tracked_secret` warning. The shell
tool's `doctor` refuses a scope file with a key the schema does not declare.

## Failure behaviour

A malformed declaration (a version this executable does not read, an absolute pattern, a
`..` segment, a name pattern with a slash, an unknown key) refuses to start, naming the
file and the key; `doctor` fails on the same; `majordomus scope --check` exits 10 when a
path given is out. A reviewer refuses a change that reads a path without judging it.

## Verification

`apps/majordomus-cli/tests/scope.rs` and the crate's unit tests in `src/scope.rs`;
`test/cases/93_scope_policy.sh` over the built executable and the shell tool.
{% endraw %}
