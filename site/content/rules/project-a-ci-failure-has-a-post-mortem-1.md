+++
title = "Every failure of the trunk's CI has a post-mortem"
description = "Every failure of the trunk's CI has a post-mortem"
weight = 61
[extra]
kind = "rule"
slug = "project-a-ci-failure-has-a-post-mortem-1"
identity = "project.a-ci-failure-has-a-post-mortem@1"
status = "active"
source = ".ai/repo/rules/project/a-ci-failure-has-a-post-mortem.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-13 the trunk's CI concluded in failure six times before noon, from seven causes.
Each was read, explained and fixed by whichever session met it, and each explanation lived
where that session wrote it: an announcement on a board that is gone with its process, a
pull request body, a commit subject. By the afternoon a session that met a red `pages` job
had to read the job log again to learn what three others already knew, and one cause, the
unexplained `interactions` failure of the night's candidate, had been read by nobody and
was not known to be unread.

An explanation that is not a record cannot be searched, counted, linked from the component
it concerns, or checked for being finished. So the explanation is a record: one kind, one
schema, one directory, read by one module of the executable and projected like every other
object of the layer. The owner's order of that day is the rule's statement.

## Requirements

- A failed or timed-out run of a workflow on the trunk is named in the `runs` of at least
  one record under `.ai/repo/post-mortems/`. A cancelled or skipped run owes nothing. One
  record may answer several runs of one cause, and one run may be answered by several
  records when it failed for several causes.
- A record is a Markdown document of kind `post-mortem` that satisfies
  `majordomus.post-mortem/v1`: the front matter the schema closes, and the sections What
  failed, Root cause, Fix and Prevention, none of them empty.
- The root cause is analysed and stated: `cause.class` and `cause.summary` in the front
  matter, the account in the body. A record whose cause is not established is `draft` with
  the class `undetermined`, and each of its sections says what is not established. A
  sentence nobody verified is written as unverified.
- An `analysed` or `resolved` record references its components — the files, cases and
  rules the cause lives in — and at least one signal: the text a failing log shows.
- Every reference of a record is a reference of the layer, `<kind>:<identity>`, that
  resolves: `components`, `prevention` and `related` are edges of the one graph, so a
  record is reached from the issue, milestone, decision, intent, claim, rule, file or case
  it names, and names them back.
- A `resolved` record names what removed the cause in `fixed_by`, and no other record does.
- The records are read by the executable and by nothing else: `majordomus postmortem list`,
  `check`, `owed`, `diagnose` and `draft` are capabilities of the one registry, so the
  command line, HTTP, MCP and the UI screen are projections of them, and the published site
  carries one page per record from the publication's own declaration.
- Troubleshooting starts from the records: `majordomus postmortem diagnose` searches the
  text of a failure for the signals of every record and names the records that match,
  before anybody reads a log by hand.

## Enforcement

`majordomus postmortem check` is the gate `post-mortems` of the structure job and refuses
an inconsistent record. The index refuses a record that does not satisfy its schema, on
every build. `majordomus postmortem owed` asks the forge for the trunk's failed runs and
exits 10 while one of them is past its grace with no record naming it. The two cases above
run all three against fixtures: a record of each status, each rule of the check removed in
turn, and a list of runs read from a file in place of the forge.

## Non-goals

- A failure on a pull request's head. It blocks its own merge and concerns its author; the
  rule is about the trunk, where a red run concerns everybody.
- Blame. A record names components and commits, never a person or a session.
- A second troubleshooting catalogue. `majordomus why diagnose` matches product pains
  against moments; this rule adds the failures of this repository's own CI and no weighting.
{% endraw %}
