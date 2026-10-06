---
schema: domain/v1
id: governance
kind: domain
title: Governance
headline: "The rules are written once, in the repository, projected into every tool's instructions, and a blocking rule stops the command instead of advising it."
problem: "Rules drift. Each tool keeps its own instruction file, and a rule nobody wired into enforcement is only a suggestion."
status: stable
weight: 30
tags: [governance, policy, rules]
---

# Governance

What must hold while work is done: the one policy every provider's instruction file is
generated from, the rules and the doctrine that says which of them a machine decides and
enforces, and the commit policy every message is judged against. A rule is blocking or
advisory, and the difference is a fact the tool reports rather than a word in a document.

Not in this domain: what proves a statement about the product, which is evidence; and the
contract a task is finished against, which is completion.
