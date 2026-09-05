---
schema: context/v1
id: ai.repo.templates
kind: context
title: Carried-over templates
description: Documents a migration brought from the pre-.ai layout because a person had changed them.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Carried-over templates

`majordomus migrate` compares every template of the old layout against the tool's own copy.
An identical file is dropped, because the tool ships it; a file that differs is moved here,
because somebody changed it deliberately and a migration may not throw that away.

Nothing reads this directory. The tool's templates live in its distribution, and the records
they seed — the decisions and open-questions stores — are created from there. A file here is
a person's edit waiting for a decision: fold it into the document it belonged to, or delete
it. An empty directory is the expected end state.
