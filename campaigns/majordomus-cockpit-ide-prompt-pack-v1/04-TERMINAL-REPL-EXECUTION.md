# Prompt 04 — Terminal, REPL, Streaming Execution, and Command Surface

## Mission

Make Cockpit capable of real development interaction, not merely inspecting registry rows.

The execution surface must be an authenticated/local-safe projection of canonical execution capabilities, not a browser shell bolted straight onto `std::process::Command`.

## First inspect

Determine what already exists for:
- execution records;
- live events;
- streaming output;
- cancellation;
- command execution;
- `just`;
- `majordomus` CLI;
- workflow execution;
- raw provider/model invocation;
- PTY support;
- subprocess policy;
- effect/risk classification.

## Implement a generic execution console

A developer should be able to launch allowed canonical operations from Cockpit and observe:

- queued/start/running/succeeded/failed/cancelled;
- stdout/stderr/event stream;
- structured progress;
- child steps;
- elapsed time;
- actor;
- working directory/worktree;
- environment profile (never secret values);
- result/artifacts;
- diagnostics;
- reproduction command;
- stable execution URL.

## `just` integration

Do NOT maintain a Cockpit list of `just` recipes.

Introspect the existing canonical mechanism. If the repository uses `just --dump --dump-format json`, consume/normalize it through a backend registry.

Requirements:
- namespaced recipes;
- descriptions;
- parameters;
- safe invocation;
- completion/search;
- execution history;
- reproduction command;
- generated docs where useful.

## Majordomus CLI bridge

Where a CLI command is a projection of a capability, run the capability directly through the executor, not by shelling back into Majordomus.

Only genuinely CLI-local commands may use a CLI-local execution bridge, and the existing local-command waiver mechanism must justify them.

## PTY / shell

If a true PTY is implemented, treat it as a separate high-risk capability with:
- explicit local-only/security policy;
- cwd/worktree restriction;
- secret redaction;
- origin/CSRF protection;
- session lifecycle;
- output limits/backpressure;
- cancellation/termination semantics;
- no implicit remote exposure;
- tests.

Prefer structured capability execution over unrestricted shell.

## Streaming protocol

Reuse the existing live execution event channel if present.

Do not add Socket.IO plus WebSocket plus SSE because someone discovered three technologies.

Select/reuse one canonical channel and document:
- event schema;
- ordering;
- reconnect/replay;
- execution cursor;
- completion;
- error;
- backpressure;
- retention.

## Acceptance

Prove in tests:
- browser execution and CLI/MCP/HTTP execution hit the same executor;
- output/events are ordered;
- reconnect restores execution state;
- cancellation obeys capability declarations;
- a completed execution has a stable deep link;
- secrets are not emitted;
- shell/PTY, if present, is not accidentally exposed through unrestricted HTTP/MCP.
