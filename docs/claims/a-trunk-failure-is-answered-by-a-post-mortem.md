# A failed run of a workflow on the trunk is answered by a typed post-mortem record or named as owed; postmortem owed exits 10 while a failed run past its grace has no record, a printed draft is a record the tool accepts, and the text of a failure seen before is answered by the record whose signal it carries

## What it means

When the trunk's CI goes red, the explanation is an object of the layer and not a message
somebody wrote somewhere. The tool can say which failed runs have such a record and which
do not, and it can be handed the text of a failure and name the records that already
explain it.

## How it works

A post-mortem is a Markdown record under `.ai/repo/post-mortems/` of kind `post-mortem`
(`share/schemas/majordomus/post-mortem/post-mortem.v1.proto`). Its front matter names the
runs it answers, a cause class and sentence, the components the cause lives in, and the
signals a failing log shows. `apps/majordomus-cli/src/postmortem/mod.rs` holds four
readings over those records:

- `judge` and `judge_body` refuse a record that does not hold together: a draft that
  claims a cause, an analysed record with no component or signal, a resolved one with no
  fix, a file or case the tree does not hold, a required section that is missing or empty;
- `owed` compares the trunk's failed and timed-out runs with the runs the records name,
  and marks each unanswered run as within or past the grace;
- `diagnose` searches a text for every record's signals, ignoring case;
- `draft` writes the text of a `draft` record for one run.

`majordomus postmortem owed` and `draft` ask the forge through `gh`, or read the runs from
a file in the forge's own listing's shape.

## How to see it

`majordomus postmortem list` prints the records, `check` judges them (exit 10 on a
finding), `owed` prints the answered and the owed runs (exit 10 past the grace) and
`diagnose` names the matching records for a text on standard input.

`test/cases/3000_a_trunk_failure_owes_a_post_mortem.sh` gives the tool one record and four
runs: the answered one is not listed, the unanswered one is owed and refuses only past its
grace, the cancelled one and the one from before the rule owe nothing; the draft printed
for the owed run is accepted and answers it; and a failure's text names the record by its
signal. `test/cases/2999_a_post_mortem_is_a_typed_record.sh` breaks one clause of the
judgement at a time.

## What it does not cover

It does not write the root cause: the tool derives the run, the jobs and the step, and a
person or a session establishes the cause, or records that it is not established. It does
not ask the forge on its own; until the trunk's workflow and a session's start run `owed`,
a failed run is named as owed only when somebody asks. A failure on a pull request's head
is outside it. A run the forge reports as successful after a rerun owes nothing, whatever
its first attempt did. And `diagnose` is a substring search: a signal too short matches
failures it does not explain, and a failure whose text changed is not found.

## Why it exists

On 2026-10-13 the trunk's CI failed six times before noon from seven causes, and every
explanation was written on a peer board, in a pull request body or in a commit subject.
One failed job had been read by nobody and nothing said so. The owner ordered that day
that every CI failure has a post-mortem kept in the repository
(`project.a-ci-failure-has-a-post-mortem`, ADR 0156).
