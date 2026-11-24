+++
title = "A record cites a campaign brief where the repository keeps it, never where it was staged"
description = "A record cites a campaign brief where the repository keeps it, never where it was staged"
weight = 62
[extra]
kind = "rule"
slug = "project-a-brief-is-cited-where-it-is-kept-1"
identity = "project.a-brief-is-cited-where-it-is-kept@1"
status = "active"
source = ".ai/repo/rules/project/a-brief-is-cited-where-it-is-kept.v1.md"
+++
{% raw %}

## Rationale

[A campaign brief this repository keeps is the one it was handed](@/rules/project-a-brief-is-kept-as-received-1.md)
holds the bytes of every pack under `campaigns/`. It says nothing about the time before a pack
gets there, and that is where packs were lost. A pack arrives as a download or a paste, is
unpacked into the primary checkout's `tmp/`, and is worked from while it is still being
written: a draft, a second version, an audit an agent writes beside it. `tmp/` is ignored and
exists in one checkout on one disk. A record planned from such a pack that cites the staging
path points at something no other worker can open.

ADR 0236 names the staging place, `tmp/owner-packs/<pack>/` in the primary checkout, with
`tmp/prompt-packs/` read as its legacy, and says when a pack moves from there to `campaigns/`:
before the first record or ADR cites it. This rule is that moment, made checkable.
`scripts/ci/campaigns-check` step 5 reads every file under `.ai/repo/project/` and
`.ai/repo/adrs/` for a citation of either staging place and fails each one the list
`.ai/repo/ci/campaigns-staged-citations.txt` does not name. A line of the list whose citation
is gone fails too, so the list can only shrink. Its first two lines are I2460 and I2461, which
cite an audit written beside the pack of 2026-10-11 and never kept.

`test/cases/4662_a_brief_is_cited_where_it_is_kept.sh` plants each branch: a citation in an
issue and in an ADR, the legacy place, a citation ending a sentence, an allowed line, a stale
allowed line, and `tmp/packs/`, which `majordomus pack` writes its output to and which a
record may describe.

## What this rule does not say

It does not decide what lives in the staging place, how long a pack may stay there, or
whether it is ever kept. A pack nobody plans from owes nothing. It does not read `docs/`, the
site, or `campaigns/README.md`, whose "taken from" column records where a kept pack came from:
that column is history, not a citation by a record.
{% endraw %}
