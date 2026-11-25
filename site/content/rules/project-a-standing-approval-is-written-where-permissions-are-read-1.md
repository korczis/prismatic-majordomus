+++
title = "A standing approval is written where the permission layer reads it"
description = "A standing approval is written where the permission layer reads it"
weight = 78
[extra]
kind = "rule"
slug = "project-a-standing-approval-is-written-where-permissions-are-read-1"
identity = "project.a-standing-approval-is-written-where-permissions-are-read@1"
status = "active"
source = ".ai/repo/rules/project/a-standing-approval-is-written-where-permissions-are-read.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-14 the owner approved, in a session and for the future, two acts the client's
permission classifier had refused: copying the shared derive-offload script into the
session's own scratchpad, and checking or stopping by exact pid a process the session had
started itself. The words, said in Czech, were "you have my permission, explicit approval,
enforced for the future too, continue". The session recorded the approval in its memory and
went on.

The next session met the same refusal. A memory is read by the model; the permission layer
does not read it, so an approval kept there is a note about a permission and not a
permission. The session had also concluded that a guard hook forbade it to change the
permission configuration at all. Read in full, the guard refused three things only: a
shell command that writes the configuration, an edit that removes the guard's own wiring,
and any change naming the guard's files. An edit that added a rule was never refused. On
2026-10-15 the owner said, in Czech, "solve this systemically too" and then "write it down
as a rule".

## Required behaviour

1. **The approval is the owner's own words, standing, and recorded.** It is quoted with its
   date where the rule is written and in the worker's report. A relayed approval, an
   inference from silence or an approval for one act is not standing; see
   `project.an-expedited-landing-is-the-owners-word` clause 1.
2. **Same turn, same place the refusal came from.** The worker writes the approval into the
   configuration its client's permission layer reads, at the scope that covers every
   checkout it works in (for a client with user and project settings, the user scope, so
   that linked worktrees and background jobs inherit it).
3. **Narrow.** The rule names the act, the paths, and the limit: "a process this session
   started, by exact pid", never "processes"; "a copy into the session's own scratchpad",
   never "copies". An allow-list prefix is used only for a command that only reads; an act
   that changes anything is written as a described rule the classifier weighs.
4. **A file edit that changes no guard.** The configuration is edited with the client's
   file-editing tool, never a shell command that rewrites it. Nothing that wires, configures
   or implements a guard is touched. A guard that refuses the edit has decided: the worker
   stops, names the refusal, and gives the owner the exact change to make; it does not
   rephrase the command until the guard stops seeing it, and does not hand the act to
   another session.
5. **Validated by a read.** The worker parses the configuration without writing it and
   shows that the new rule is present and every existing guard is still wired.
6. **Memory follows the rule, not the reverse.** A memory may point at the rule; it is never
   the only record of the approval.

For Claude Code this is `autoMode.allow` (described rules, `"$defaults"` kept first) and
`permissions.allow` (read-only command prefixes) in `~/.claude/settings.json`, edited with
the Edit tool; a project's `.claude/settings.local.json` does not reach a linked worktree.

## Failure behaviour

An approval found only in a memory or a handover is written into the permission
configuration by the next worker that reads it, before it acts on it, or reported to the
owner as unwritten. A refusal by a guard is reported with its text and is not retried in
another form.

## Verification

Review of the worker's report: the quoted approval, the rule as written, and the read that
shows it parsed with the guards still wired.
{% endraw %}
