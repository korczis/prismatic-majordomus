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
block is not therefore unenforced; `class` still says what a violation means, and there are
two ways it can hold.

A rule can be held by a **gate this repository declares**. `project.derived-files-regenerated`
names no validator, because which files are canonical inputs and where their hash is recorded
are facts about this repository and not about every installation of the tool — a validator in
`lib/` would teach the shipped executable one repository's layout. Instead the policy's
`enforcement` list names `derived-current`, wired by `git-hook:pre-commit`, and `doctor`
proves that wiring the way it proves `doctor-on-commit`: the hook must exist, be executable,
invoke the gate, and not swallow its exit code. The gate refuses the offending commit; the
doctrine refuses the repository that unwires the gate. `majordomus rules list` says
`no validator; see the rule` for such a rule, which is a statement about where the check
lives, not about whether one exists.

A rule can also be held by **a reader**, when the tool cannot decide the question at all.
Write that kind deliberately: inventing a validator that always passes is worse than
admitting a person owns it, and so is leaving a rule looking unenforced when a gate holds it.
