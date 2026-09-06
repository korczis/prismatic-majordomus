---
schema: moment/v1
id: milestone-status-unreconstructable
kind: moment
title: 'Nobody can reconstruct where the milestone actually is'
short_title: 'Unreconstructable status'
hook: 'was asked how far the milestone had got and had to go and ask four people'
summary: 'Progress is an aggregate that exists only in people, because the parts it aggregates were never recorded in a form anything can add up.'
status: stable
severity: medium
frequency: common
weight: 230
audiences: [engineering-lead, enterprise, platform-team, agency]
areas: [work-tracking, observability]
lifecycle: [planning, operations]
tags: [milestones, status, derivation, reporting]
signals:
  - id: ask-four-people
    text: 'Answering "how far along is this" requires asking several people.'
  - id: percentage-invented
    text: 'Progress is reported as a percentage that nobody can show the arithmetic for.'
  - id: no-derivable-state
    text: 'The repository cannot say what is finished without a person interpreting it.'
examples:
  - id: monday-report
    audience: engineering-lead
    title: 'The Monday number'
    before: 'A progress figure is assembled from three conversations and presented as though it were measured.'
    after: 'Progress is derived from the issues that name the milestone and the events they recorded; the arithmetic is visible.'
  - id: quarterly-review
    audience: enterprise
    title: 'A quarter that cannot be reconstructed'
    before: 'Six months later, nobody can say when a milestone was reached or on what basis it was called reached.'
    after: 'The milestone declares the evidence its outcome requires, and each piece is recorded with the command and the commit that produced it.'
  - id: client-update
    audience: agency
    title: 'The client update'
    before: 'Status for a client is reconstructed weekly, by hand, from memory and branch names.'
    after: '`plan` reads the model and reports the derived state; the report is the same one the team reads.'
commands: [plan, history, doctor]
capabilities: [graph.get, graph.list]
claims: [project-status-derived, execution-waves, evidence-gates-done, roadmap-derived, history-ledger-read]
doctrines: [majordomus.project-integrity, majordomus.roadmap-integrity, majordomus.dag-integrity, project.no-counts-in-prose]
use_cases: [plan-the-work-as-data, deliver-issues-in-waves, read-back-what-happened]
related: [three-roadmaps-none-of-them-true, issue-says-done-tests-disagree, what-the-workers-did-last-night]
aliases: ['progress cannot be measured', 'status by interview', 'invented percentage']
---

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
