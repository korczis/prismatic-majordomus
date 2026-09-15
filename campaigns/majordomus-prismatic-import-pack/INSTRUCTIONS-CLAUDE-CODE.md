# Claude Code Usage Instructions

## Preferred model/context

Use the strongest available Claude Code model with the largest practical context window.

Do not economize context during Prompt 01. The entire value of this migration depends on understanding both repositories, their instructions and the actual implementation graph.

## Start

```bash
cd ~/dev/prismatic-majordomus
git status --short --branch
claude
```

Feed `00-CONTEXT.md` once as shared context if your Claude Code workflow does not automatically preserve files.

Then execute prompts 01–08 sequentially.

## Before every prompt

Claude must:

```bash
git status --short --branch
```

and inspect any repository-specific session/handover mechanism.

It must preserve unrelated changes.

## Source repository access

The donor is:

```text
~/dev/prismatic-platform
```

Treat it as read-only unless there is an extremely strong, explicit reason otherwise.

The target is:

```text
~/dev/prismatic-majordomus
```

All migration implementation belongs in the target.

## Suggested branch/worktree behavior

Follow Majordomus' current canonical worktree doctrine if present.

Do not invent a conflicting worktree layout.

If target doctrine requires one feature branch + sibling worktree, comply with it.

## Commit strategy

Prefer coherent architectural commits, e.g.:

1. canonical model/schema/provenance;
2. skill imports;
3. doctrine/rule enforcement;
4. legacy migration/dedup;
5. surface integration;
6. tests/docs/gates.

But defer to repository conventions.

Never squash away useful provenance if repository history is part of auditability.

## Failure handling

If a prompt discovers that a previous architectural assumption is wrong:

- update the migration matrix;
- adapt the design;
- do not preserve a bad abstraction merely because an earlier prompt introduced it.

If tests reveal pre-existing unrelated failures, record them precisely with evidence and continue validating changed scope.

## Strong prohibition

Do not use the prompt pack as permission to import all Prismatic content.

Selection is evidence-based.

The target should become stronger and simpler, not merely heavier.
