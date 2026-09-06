---
schema: audience/v1
id: ai-native-team
kind: audience
title: AI-native development team
short_title: AI-native team
summary: 'Several people and more assistants than people, working the same repository at the same time.'
workflow: 'Claude, Codex and Gemini sessions in parallel worktrees and branches, work handed between humans and workers, and no single place that says who holds what.'
status: stable
weight: 20
tags: [agents, teams, parallelism]
---

# AI-native development team

## Who this is

A team that has already reorganised around assistants rather than merely adopted them.
Two to ten people, and at any moment more sessions than people: coding agents in
worktrees, review agents on branches, overnight runs nobody watches.

## How they work

Work is fanned out. A person states an outcome, several workers attempt parts of it, and
somebody integrates. The bottleneck moved: it is no longer typing, it is knowing what the
other workers already did, already decided, and already broke.

## What goes wrong

Duplicated work that looks like parallelism. One worker undoing another's change because
neither could see the other's scope. A repository where three sessions each believe they
own the same file. Isolation — a worktree each — defers the collision without preventing
the duplication, because what prevents duplication is a declared, visible scope.
