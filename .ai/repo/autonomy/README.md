---
schema: context/v1
id: ai.repo.autonomy
kind: context
title: Standing approvals
description: The owner's standing approvals for an agent client, declared once for every machine; `majordomus autonomy approvals` says whether a machine holds them and `autonomy apply` writes what it lacks.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/autonomy-declaration/autonomy-declaration.v1.schema.json, share/kinds.yaml, docs/AUTONOMY.md]
---

# Standing approvals

`claude-code.yaml` is the record of every approval the owner gave a Claude Code session for
the future, each with its date and the rule exactly as the client's auto-mode classifier
reads it, the environment lines the classifier is told, and the few settings beside them.

The client reads its permissions from each machine's `~/.claude/settings.json`, so an
approval written only there is held on one machine and asked for on every other. On
2026-10-20 the owner's MacBook asked for what the other MacBook had been granted a week
before; this declaration is the answer. A worker that turns a new standing approval into a
rule (`project.a-standing-approval-is-written-where-permissions-are-read`) adds it here in
the same change, and every machine then holds it after one `majordomus autonomy apply`.

`majordomus autonomy approvals` compares this machine's user settings with the declaration
— each approval held or missing, the environment lines and the settings — and the brief
says the verdict on every machine. `majordomus autonomy apply` writes only what is
declared and missing, keeps every other key, entry and hook, and leaves a copy of the file
as it was. It is a command a person runs; no projection offers it.

The schema is `autonomy-declaration/v1`
(`share/schemas/majordomus/autonomy-declaration/autonomy-declaration.v1.schema.json`),
validated on every index build; the typed reading is
`apps/majordomus-cli/src/autonomy/approvals.rs`. The repository is public: nothing secret
belongs here, and an approval that would need one is not declared.
