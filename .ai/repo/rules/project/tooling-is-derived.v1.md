---
id: project.tooling-is-derived
version: 1
kind: rule
title: AI client instructions are derived from canonical repository metadata, never maintained by hand
description: The instruction file each AI client reads — AGENTS.md, CLAUDE.md, every projection the policy declares — is generated from the policy and the shipped declarations, stamped with the hash of what produced it, and refused when edited by hand or when its source has moved. What the files say about the lifecycle, and in particular the definition of done, is a fragment rendered from share/completion.yaml by both renderers to the same bytes. A capability inventory, a command list or a lifecycle stated in a client file by hand is the defect; policy changes by changing the source and regenerating, never by editing a projection.
statement: Generate every AI client instruction file from the policy and the shipped declarations, render the definition of done from share/completion.yaml as a fragment both renderers produce byte for byte, stamp each projection with the hash of its source and its content, refuse a hand edit and a stale render, and never let a generated file become the place a policy is changed.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.completion-is-proved@1]
tags: [providers, projections, governance, agents]

x-majordomus:
  tests: [test/cases/282_tooling_is_derived.sh, scripts/ci/providers-check]
---

# Rationale

Two AI tools' instruction files in one repository once shared 0–9 % of their content
(README, "The problem"). This repository generated its bootstraps from the policy from the
start and still carried the lifecycle in them as prose, from templates that duplicated
whole paragraphs by hand; the fragment machinery that could have rendered a contract into
them was built and used by nothing. A definition of done that lives in a prompt is
unversioned, unenforceable, and true for exactly one tool.

The direction matters as much as the mechanism. A model that edits `AGENTS.md` has not
changed policy; a policy changes in `.ai/repo/policy.yaml` and `share/`, under review, and
the projections follow. A generated file that becomes the place a rule is written is the
drift this layer exists to prevent.

# Required behaviour

- Every projection the policy declares is rendered from its template, the policy and the
  shipped declarations, and carries a stamp naming the policy hash and the hash of its own
  content. `majordomus update` and `majordomus generate providers` render the same bytes.
- The definition of done reaches every bootstrap as `{{COMPLETION_CONTRACT}}`, rendered
  from `share/completion.yaml` — one line per stage with the questions that belong to it —
  by `mj_completion_fragment` and `CompletionPolicy::bootstrap_fragment` identically. A
  stage or a question added to the policy appears in every bootstrap on the next render.
- `majordomus doctor` refuses a projection whose content no longer matches its stamp (a
  hand edit) and `majordomus generate --check` refuses one whose source has moved (a
  stale render). Regeneration is the remedy in both cases, and it is the only one.
- No bootstrap carries a rule of its own, a capability inventory, a command list or a
  lifecycle written by hand; it points at where those are declared.

# Failure behaviour

A hand-edited projection fails `doctor` (`hand_edited`); a projection whose source moved
fails `generate --check` and the `providers-data` gate; a template whose fragment token
the renderers fill differently fails the same check, because the stamp of one render
cannot match the other. A rule or an inventory found only in a bootstrap is a violation
of `project.interfaces-are-projections` and of this rule.

# Verification

`test/cases/282_tooling_is_derived.sh` proves the fragment reaches the rendered bootstrap,
that the shell renderer and the executable agree byte for byte, that a change to the
policy's stages makes the committed render stale until regenerated, and that a hand edit
is refused. `scripts/ci/providers-check` holds the declarations and the templates together.
