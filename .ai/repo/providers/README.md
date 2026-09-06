---
schema: context/v1
id: ai.repo.providers
kind: context
title: Provider templates
description: The one source each provider bootstrap is rendered from; the files at the root are output.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [lib/update.sh, .ai/repo/policy.yaml]
---

# Provider templates

`<provider>.tmpl` is what the bootstrap file at the repository root is rendered from.
`majordomus update` renders each template the policy's `projections` list names, stamps
the result, and `doctor` fails on a stamp that no longer matches its source: the root
files are output, and editing one there is drift the next check reports.

A bootstrap says how to find the policy, never what the policy is. It points at
`.ai/README.md` and the rules, keeps inside the line budget the policy sets, and carries
no rule corpus of its own — the doctrine that guards this refuses a provider file that
starts collecting rules, because two copies of a rule is one rule and one lie.

Adding a provider is adding its template here and its projection to the policy; nothing
else registers it.
