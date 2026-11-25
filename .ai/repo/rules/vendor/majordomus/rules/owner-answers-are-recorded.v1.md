---
id: majordomus.owner-answers-are-recorded
version: 1
kind: rule
title: An owner's answer is a tracked, verbatim record, and what cites it resolves
description: An answer the repository's owner gives to a question a worker put is recorded in the repository, in the same session, as an owner-answer record holding the question, the options and the answer verbatim with who asked, who recorded and how the text was obtained. A record holds the text its digests name, and every owner-answer:<id> cited under the layer resolves.
statement: An owner's answer is recorded with `majordomus question record` as .ai/repo/answers/<id>.yaml in the session that received it, verbatim, with its provenance stated; it is cited as owner-answer:<id> and never retyped, a record is never edited, and a citation that names no record is reported.
status: active
class: advisory
depends_on: [majordomus.externalise-decisions@1]
tags: [records, decisions, owner]

x-majordomus:
  validator: owner_answers
  category: records
  enforced_by: [check, doctor, watch]
  exit_code: 0
  claims: []
  tests: [test/cases/4669_an_owner_answer_carries_its_provenance.sh]
---

# Rationale

An owner's answer settles decisions, pull requests, plan issues and ADR acceptances, and it
is the one input to a repository that no command can reconstruct. Recorded in a worker's
memory, in the untracked half of the layer or retyped into prose, it is lost with the
checkout. A record that only resembles it is not verbatim either. In one adopter repository
an answer was relayed between sessions. In transit the answer gained a claim that the
question had been translated, and its free text lost one of its two spaces. Nothing could
tell the copy from the original. So the answer is a record of its own kind:

- the text is held byte for byte;
- the digests are recomputed on every check;
- the record states how strong its provenance is, whether checked against the provider's
  transcript or typed by the recorder, instead of claiming proof it cannot have.

# Required behaviour

- In the session that receives an owner's answer, record it with `majordomus question
  record`. Prefer `--transcript <file> --locator <id>`, which reads the question, the
  options, the asking session and both times from the provider's transcript and checks the
  answer against its result. Typed text is recorded as `text_source: typed`.
- Commit the record with the work it settles, and name that work in `--settles`.
- Cite it as `owner-answer:<id>`; never retype its text in another record.
- Never edit a record. A correction is a new record with `--supersedes <id>`.

# Failure behaviour

A violation is a `WARN` finding under the category `records`, and the command continues and
exits as it otherwise would. These are violations:

- a record whose question or answer is no longer the text its digest names;
- a record that selects a label it did not offer;
- a record that claims `transcript-checked` without a locator and a digest;
- a citation `owner-answer:<id>` under the layer's repository half that names no record.

Under `watch` a violation is reported as drift and the command exits 11.

# Verification

`mj_validate_owner_answers` decides it, dispatched from `check, doctor, watch`. The behavioural
case `test/cases/4669_an_owner_answer_carries_its_provenance.sh` proves it, and CI runs that case.
