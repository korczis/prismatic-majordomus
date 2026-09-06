---
schema: context/v1
id: ai.repo.rules.project
kind: context
title: Project rules
description: The rules this repository wrote for itself, one file each, resolved with the vendored baseline.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Project rules

The rules this repository wrote for itself. The format, the loading and the composition
are the section's, in `../README.md`, and are not repeated here; what this directory adds
is where a project rule may go and where it may not.

A file here is `<slug>.v<version>.md`, and the file name is a convenience: identity is the
`id` and `version` in the front matter, namespaced `project.*`. A project rule may add a
constraint the vendored baseline does not carry. It may not reuse a vendored identity,
weaken a vendored rule, or exist only to restate one — the effective set is additive and
has no override mechanism, so a rule that contradicts its baseline is two rules in force
at once.

A rule with an `x-majordomus` block claims the tool enforces it, and that claim is checked
in both directions: the validator function must exist, the commands named in `enforced_by`
must dispatch it, and the tests and claims it names must resolve. A rule without that
block is normative for whoever reads it and enforced by a reviewer; `class` still says
what a violation means. Write the second kind when the tool cannot decide the question —
inventing a validator that always passes is worse than admitting a reviewer owns it.
