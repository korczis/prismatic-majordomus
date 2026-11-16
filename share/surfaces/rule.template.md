---
id: project.published-surfaces-are-derived
version: 1
kind: rule
title: Every published surface is derived, and a stale region or a dangling link blocks the change
description: The site, the wiki and the generated Markdown regions are produced by the surfaces producer from tracked Markdown, git and the .ai/ layer. Nothing published is hand-written, and surfaces check passes before a change lands.
statement: Publish documentation only through the surfaces producer, edit the source document and never a generated region, page or wiki entry, and keep surfaces check passing (no stale region, no relative link or anchor to nothing).
status: active
class: blocking
depends_on: []
tags: [documentation, publication, derivation]
---

<!-- Template shipped by Majordomus in share/surfaces/rule.template.md. Copy it to
     .ai/repo/rules/project/published-surfaces-are-derived.v1.md when adopting the producer
     (share/surfaces/README.md, "Adopting it in a repository") and delete this comment. It
     carries no x-majordomus block: the pre-commit hook, the repository's gate and CI run the
     check, so a doctor validator would only restate them. -->

# Rationale

Any published copy that a person keeps in step will drift. A hand-written landing page
restates the README until the two disagree. A status table says "planned" after the work
merged. An index misses the newest document. Derivation removes the second copy, and the
check turns any drift that remains into a failure with a file and a line.

# Required behaviour

- A new fact about the repository goes into the document or the `.ai/` record that owns
  it. The site, the wiki and every `surfaces:begin`/`surfaces:end` region pick it up on
  the next build or `tools/surfaces/surfaces sync`.
- The text between region markers is never edited by hand. Change the generator, its
  arguments, or the source it reads.
- A relative link points at a tracked file or directory, and an anchor names a heading or
  an explicit anchor in the target. A document whose links are someone else's to keep is
  exempted by path in `surfaces.toml` (`[check] exclude`), with its reason.
- The producer stays generic. Every repository-specific choice lives in `surfaces.toml`.

# Failure behaviour

`tools/surfaces/surfaces check` exits 10 and names each finding as `FAIL <kind>
<path>:<line> — <message>`. It runs, and blocks, in three places:

- the pre-commit hook (policy `enforcement`, entry `surfaces-on-commit`, reconciled by
  `majordomus doctor`);
- the repository's own gate;
- `.github/workflows/surfaces.yml`, which publishes nothing from a failing run.
