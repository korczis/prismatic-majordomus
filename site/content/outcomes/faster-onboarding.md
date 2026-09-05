+++
title = "Faster onboarding"
description = "Make every developer and AI agent productive faster."
weight = 1
[extra]
moments = ["re-explaining-context", "rule-in-a-readme-nobody-loaded", "two-rulebooks-one-repository"]
commands = ["init", "update", "context", "handover"]
claims = ["projection-generation", "bootstrap-chain", "context-assembly", "handover-record"]
+++
{% raw %}
## The situation

Onboarding is not a week on the calendar. It is the time between someone joining a piece of
work and their first useful contribution, and it happens far more often than hiring does.
Every new AI session is an onboarding. So is every switch to a different model, every second
tool opened on the same repository, and every developer picking up a task someone else left.

Each of those starts by learning the project again: reading the repository, asking which
rules apply, rediscovering decisions that were already made and why. The senior people
answer the same questions each time, and the AI spends its first effort re-reading what the
last session already read.

## What Majordomus changes

The project keeps what it knows in one place that every worker reads. One policy describes
how work is done here, and the instruction file each AI tool expects is generated from it,
so two tools open on one repository read the same rules rather than two rulebooks. The
context a worker needs is assembled from durable records in a fixed order, so a session gets
what applies to its task and nothing it does not. Decisions are recorded with their reason,
and a handover is a record the next worker can trust against the repository, not a chat
transcript.

A newcomer, human or AI, therefore starts from what the team already established. The
questions that used to be asked again have answers on disk, and the answers are the same
whichever tool or model is asking.

## Where the time and money go

The cost of repeated onboarding is paid twice: in the hours of the people who explain, and
in the AI effort spent rebuilding context before any work starts. Majordomus removes the
repeated part. It does not shorten the first onboarding; it stops the second, third and
hundredth from starting at zero.

## What this does not promise

Majordomus does not write your documentation and does not make a model understand a
project it has never seen. It carries forward what the team chose to record, in a form every
worker reads. The pages linked below say exactly which commands do this and what is
guaranteed about each.
{% endraw %}
