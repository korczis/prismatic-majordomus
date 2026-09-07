+++
title = "Every use case is executed against the tool, not described"
description = "A use case names the commands, rules, claims and applications it relies on and carries a scenario as data; usecase run executes it in a disposable repository and records normalised evidence; maturity is observed from the evidence; coverage of commands, guaranteed claims and MCP tools is tallied and the policy says which gaps fail."
weight = 150
[extra]
id = "use-cases"
status = "stable"
source = ".ai/repo/features/use-cases.md"
+++
{% raw %}

## What it does

`majordomus usecase run` executes every scenario; `usecase coverage` tallies every public
command, guaranteed claim and MCP tool against the use cases that name and run it;
`usecase impact` says what a change reaches; `usecase scaffold --missing` writes a draft for
each gap from what the tool already knows. An application says when the tool fits and,
because a catalogue that only lists fits is marketing, when it does not.

## What it does not do

A draft never counts until it is made active, and a status written by hand is refused: the
maturity of a use case is what its evidence supports.
{% endraw %}
