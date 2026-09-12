+++
title = "A request never rebuilds canonical state, and a cache is never trusted without an equivalence test"
description = "A request never rebuilds canonical state, and a cache is never trusted without an equivalence test"
weight = 118
[extra]
kind = "rule"
slug = "project-rust-hot-path-1"
identity = "project.rust-hot-path@1"
status = "active"
source = ".ai/repo/rules/project/rust-hot-path.v1.md"
+++
{% raw %}

## Rationale

The slow path the mission started from was not a slow algorithm but work done per request
that belonged at startup. Counters make that class of regression a failing test rather
than a feeling; one executor makes instrumentation and caching apply to every transport
at once; and a cache that is not proved equivalent is a lie waiting for an input.

## Required behaviour

`apps/majordomus-cli/tests/hot_path.rs` sends hundreds of requests through the real
stdio and HTTP transports and requires the startup counters unchanged; every new transport
or route dispatches through `Context::execute`; a new `CachePolicy::Process` is added to
the descriptor and nothing else, and `tests/executor.rs` and `tests/properties.rs`,
which iterate every cached capability of the registry, pass.

## Failure behaviour

`tests/hot_path.rs` fails naming the counter that moved; the registry refuses a cached
command or a policy that keeps nothing; the equivalence tests fail naming the capability
and the case.

## Verification

`scripts/rust-check`; `apps/majordomus-cli/tests/{hot_path,executor,properties}.rs`;
`test/cases/91_canonical_architecture.sh`.
{% endraw %}
