+++
title = "A long operation reports typed events, and a transport only renders them"
description = "A long operation reports typed events, and a transport only renders them"
weight = 84
[extra]
kind = "rule"
slug = "project-execution-events-are-typed-1"
identity = "project.execution-events-are-typed@1"
status = "active"
source = ".ai/repo/rules/project/execution-events-are-typed.v1.md"
+++
{% raw %}

## Rationale

The first thing a live channel attracts is logic. It has the events, it has the connection,
and it is the only place that knows a client is watching — so the temptation is to decide
there whether an execution has finished, to format a message for the browser that happens to
be attached, and to answer a second client differently because it asked differently. Every
one of those is a fact about executions living in a socket handler, where nothing else can
read it, no test that does not open a socket can reach it, and the next consumer — an audit
trail, a notifier, an agent — has to reimplement it.

The second thing it attracts is strings. A line of text is the cheapest thing to send and
the most expensive thing to consume: a browser that wants a progress bar has to parse
`17/42`, and the day the wording changes the bar stops moving with no failure anywhere. A
control plane whose progress is inferred by regular expression over human output is a
control plane that lies quietly.

Both are avoided the same way. The domain produces a typed value; one store keeps it,
orders it and hands it out; transports render. What a client validates against is then
generated from the same Rust type the handler produced, exactly as an input and an output
schema already are, and a variant added in one place appears in the protocol document, the
Cockpit and the reference together.

## Required behaviour

An operation long enough to be watched — any handler with something to say on the way —
reports through the `Progress` handle its context carries: named steps, measured progress, log lines, structured diagnostics. A handler that
looks at its cancellation flag and stops says so on its declaration (`.cancellable()`), so
that no client offers a control that does nothing. Reporting is
silent outside an execution, so a handler reports unconditionally and is written once for
every caller. A handler must not open a socket, name a client, format for a browser, or
learn which transport called it.

An execution event is a variant of the one typed enum, carried in one envelope with a
protocol version, a dense per-execution sequence and a `type`/`data` body. Its JSON Schema
is derived from the Rust type by the same path every capability's schemas take and is served
by a capability, because OpenAPI cannot describe a socket. A hand-written copy of that
schema — in a document, a template, a script or a client — is a defect.

State moves in exactly one place: the store, under its lock, refusing a transition the state
machine does not allow. No transport, page or client may compute an execution's state from
the events it happened to see; it reads the snapshot, which is produced under the same lock
and therefore cannot say `succeeded` and carry no output.

Every bound is typed and every behaviour at a bound is defined: how many executions are
remembered, how many events are retained each, how long a log line may be, how far a
subscriber may fall behind. A subscriber that falls behind is told to resynchronise and is
never waited for — an execution must not be slowed by a client watching it — and a subscriber
that has gone is forgotten at the next event rather than accumulating.

A client rejoining — after a reload, a reconnection or a gap — asks for what it missed by
sequence, and gets the retained history and then the live stream with no gap and nothing
written twice. Polling a snapshot repeatedly is what the HTTP routes are for at a reload,
a reconciliation or a disagreement; it is not the live mechanism.

## Failure behaviour

A reviewer refuses a handler that reaches for a transport, a transport that decides an
execution's state, a stream of untyped text, and a schema written by hand where one is
derived. The crate's suites decide the machine-checkable part: the state machine's
transitions, the refusal of an invalid one, the density and ordering of sequences, the
agreement of the snapshot with the stream, the bounds and the behaviour at each of them, and
the replay-from-a-cursor contract are all asserted, and `majordomus generate --check` fails
when a committed projection no longer matches the types.

## Verification

`apps/majordomus-cli/src/execution/` unit tests (the state machine, the store's bounds and
fan-out, the sink, the redaction), `apps/majordomus-cli/tests/executions.rs` (the whole path
over a real socket: start, follow, reload, reconnect with a cursor, cancel, fail), the
doctests of `execution::event` and `http::ws`, and `bash test/run.sh 100_execution_plane`,
which proves from outside the executable that a capability started over HTTP is streamed
over the WebSocket and read back identically by the snapshot, the history and the command
line. `docs/EXECUTIONS.md` is the model; ADR 22 is the decision.
{% endraw %}
