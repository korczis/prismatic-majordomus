+++
title = "Clean room"
description = "Clean room"
weight = 66
[extra]
kind = "rule"
slug = "project-clean-room-1"
identity = "project.clean-room@1"
status = "active"
source = ".ai/repo/rules/project/clean-room.v1.md"
+++
{% raw %}

## Rationale

The tool distils patterns, not code or vocabulary, from the environment it came from; docs/EXTRACTION_REPORT.md records what was taken and the relationship is one-way.

## Required behaviour

The Prismatic name is the only thing carried from anywhere else: no internal paths, hostnames, component names, doctrine vocabulary or quotations.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. Review against docs/EXTRACTION_REPORT.md.
{% endraw %}
