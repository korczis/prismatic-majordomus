---
schema: context/v1
id: ai.repo.answers
kind: context
title: Owner answers
description: The owner's answers to questions the workers put, one record each, verbatim, with who asked, who recorded and how the text was obtained.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Owner answers

One record per answer, `<id>.yaml` of kind `owner-answer` (schema `owner-answer/v1`), written
by `majordomus question record` and never edited. A record holds the question, the options
offered and the answer byte for byte, with the sha256 digests the validator recomputes. It
says how strong its provenance is (`text_source`): `transcript-checked` when the question was
found in the provider's transcript and the answer in its result, `typed` when a recorder typed
it. It names what it settles (`settles`). Another record cites it as `owner-answer:<id>` and
never retypes it; a correction is a new record with `supersedes`.

`majordomus question answers` lists them, `majordomus question answer <id>` shows one, and the
registry serves each as `majordomus://owner-answer/<id>`.
