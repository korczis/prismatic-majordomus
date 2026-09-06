+++
title = "Nobody can reconstruct where the milestone actually is"
description = "Progress is an aggregate that exists only in people, because the parts it aggregates were never recorded in a form anything can add up."
weight = 230
[extra]
id = "milestone-status-unreconstructable"
status = "stable"
source = ".ai/repo/why/moments/milestone-status-unreconstructable.md"
+++
{% raw %}

## The moment

"How far along is the migration?" The honest answer takes two days and four conversations,
and it is still an estimate. The number that gets reported is somebody's impression, stated
with a decimal point.

## Why it happens

Progress is an aggregate. Aggregating requires that the parts exist as facts — this item is
finished, this one is blocked on that one — and they do not: they exist as impressions in
the people who did the work. A tracker holds a shadow of them, updated by hand, at the
moments when someone remembered.

## Why a better model does not fix it

Asking a model to estimate progress from a repository produces a plausible number with no
derivation. The problem is not the estimate; it is that a derivable fact is being estimated
at all.

## What it costs

Planning decisions made on numbers that were felt rather than computed, and a reporting
ritual that consumes senior time every week to produce something everyone quietly discounts.

## What Majordomus does

A milestone is an outcome specification: the problem that exists today, the outcome that
ends it, and the evidence that will prove it reached. Issues reference the milestone; the
milestone references no issue, so it never has to be edited when the work under it is
re-planned. Progress is derived from the issues that name it and the events they recorded,
every time it is read. Evidence is a command or an artifact with its result and its commit,
not a sentence. The roadmap page and the tracker are projections of the same files.

## Before and after

```text
before   "about seventy per cent"       (source: a feeling)

after    $ majordomus plan
         M003  session-knowledge-integration
           DONE 7   ACTIVE 2   READY 1   BLOCKED 4   (derived, from 14 issues)
           evidence: 2 of 3 covered — 'session_nodes_reference' uncovered
```

## What it does not do

It does not estimate a date and it does not weight items by size. It reports the derived
state of the graph, which is a different and more defensible thing than a percentage.
{% endraw %}
