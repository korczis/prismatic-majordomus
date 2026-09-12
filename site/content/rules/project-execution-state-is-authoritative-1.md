+++
title = "Execution state is authoritative and the terminal is not"
description = "Execution state is authoritative and the terminal is not"
weight = 84
[extra]
kind = "rule"
slug = "project-execution-state-is-authoritative-1"
identity = "project.execution-state-is-authoritative@1"
status = "active"
source = ".ai/repo/rules/project/execution-state-is-authoritative.v1.md"
+++
{% raw %}

## Rationale

On 2026-09-10 the operator reported that sessions in this repository stop making progress
whenever the terminal tab holding them is not the focused one, and resume the instant a key is
pressed or the tab is clicked. The behaviour is not this repository's: Claude Code 2.1.267 —
the latest published version that day — is described doing exactly this in
`anthropics/claude-code` issues 36418, 38932 and 46691. The reports agree on the mechanism:
the poller that consumes a finished delegate's message is tied to the terminal UI's render
loop, so a result that is already computed and already queued waits for a keystroke to be
noticed. All three issues are closed without a fix — 36418 as a duplicate of 25068, the other
two by the staleness bot, the last live reproduction on a different operating system and a
different multiplexer.

The upstream defect is not the point. The point is the shape of it, because this repository
can build the same shape in its own work and has every incentive to: **a presentation layer
was made load-bearing for execution semantics.** Whether the delegate's work continues came to
depend on whether a human was looking at the window. That is a category error independent of
the vendor who made it, and a worker here reproduces it every time it treats what is on the
screen as the state of the world.

The repository already says the neighbouring thing about exit codes: a command that reports an
error does not exit zero, because everything downstream is entitled to believe the return
code. This rule states the general case. What a run *is* lives in its exit status, its output,
the files it wrote and the messages it sent. What is drawn in a terminal is a rendering of
that, produced for a person, and it can be stale, throttled, scrolled away, buffered or never
drawn at all without a single fact about the run having changed.

## Required behaviour

**Forward progress is decided by execution state.** A worker continues because a process
exited, a predicate over real state became true, or a result arrived — never because a key was
pressed, a pane became visible or a spinner redrew.

**Evidence is read in order of authority**: process exit status first, then structured output,
then files and state artifacts, then messages between processes, then logs, and only last a
human-facing rendering. Where two disagree, the more authoritative one is the answer and the
disagreement is itself worth reporting.

**A completed delegate's result is consumed immediately.** Work that has finished is taken up
as soon as it exists. A parent that has delegated does not stop to announce that a child
finished; it consumes the result, validates it, and continues the workflow in the same breath.

**A dependency on focus, input or rendering is a defect, not a workflow.** Where one is
discovered it is named as an implementation defect and routed around — a supervised execution,
a state file, a completion marker — rather than documented as a step a person performs. Asking
the operator to press a key to wake a machine is the last resort after programmatic recovery
has been shown not to exist, never the first suggestion.

## Failure behaviour

Decided by review. Nothing in the tool can observe another program's render loop, and a rule
about how a worker reasons is not a scan over a tree. What can be checked mechanically is the
narrower half — a command run interactively, an unbounded wait — and those are
`project.commands-run-non-interactively` and `project.every-wait-is-bounded`, which this rule
is the reason for.

## Verification

Review, and the two rules that depend on this one. A change that makes a human-facing surface
a precondition for execution is not merged.
{% endraw %}
