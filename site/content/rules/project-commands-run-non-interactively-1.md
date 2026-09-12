+++
title = "A command runs with nobody at the keyboard"
description = "A command runs with nobody at the keyboard"
weight = 67
[extra]
kind = "rule"
slug = "project-commands-run-non-interactively-1"
identity = "project.commands-run-non-interactively@1"
status = "active"
source = ".ai/repo/rules/project/commands-run-non-interactively.v1.md"
+++
{% raw %}

## Rationale

The workers in this repository are not people, and the commands they reach for were mostly
written for people. `git log` pipes itself into a pager that waits for `q`. `git rebase -i`
opens an editor. A package manager asks whether to proceed. Each of these is a small
courtesy to a human operator and a deadlock to everything else, because the program is not
broken and not slow — it is waiting, correctly, for input that will never come.

This failure is worse than a crash in a specific way: **it consumes the whole wait budget and
reports nothing.** A command that exits non-zero says what went wrong in a second. A command
blocked on a pager produces no output, no exit code and no diagnosis; from outside it is
indistinguishable from slow work, which is the ambiguity `project.every-wait-is-bounded`
exists to resolve and this rule exists to avoid creating in the first place.

The fix is not clever and does not need to be. Almost every such program has a documented
non-interactive form — an environment variable, a `--yes`, a `--no-pager`, a `--batch`, an
argument instead of a prompt. Choosing it costs nothing and removes an entire class of hang.

## Required behaviour

**The non-interactive form is the default choice.** Pagers are disabled (`PAGER=cat`,
`GIT_PAGER=cat`, `--no-pager`). Confirmations are supplied as explicit flags rather than
answered at a prompt. Where a command reads stdin, stdin is provided or closed deliberately
rather than left attached to a terminal that may or may not exist. Where a program honours
`CI`, `TERM=dumb` or an equivalent, set it.

**A command that can block for input is not started.** Interactive pagers and viewers, editors,
full-screen monitors, and REPLs are not part of an automated run. Neither is any command whose
documented behaviour includes waiting for a keypress. Where such a command is the only way to
do something, that is a finding, not a step.

**An interactive session is an explicit request.** The operator may ask for one, and then this
rule does not apply to what they asked for. Nothing else lifts it — in particular, "it will
probably be fine because the output is short" does not, because whether a pager engages depends
on the terminal, not on the output.

**Confirmation and interaction are not the same thing.** This rule requires that a
confirmation be passed as an argument, not that it be skipped. A destructive action still needs
the authority the repository requires for it; what changes is that the authority arrives as a
flag decided in advance rather than as a prompt answered by whoever happens to be watching.

## Failure behaviour

Advisory today. The mechanical form — a scan of this repository's scripts and workflows for
the interactive constructs named above, in the manner of
`test/cases/08_no_forbidden_constructs.sh` — is the follow-up recorded in ADR 0039 and would
make this rule blocking. Until it exists, review decides.

## Verification

Review. `test/cases/08_no_forbidden_constructs.sh` already scans for constructs the repository
forbids and is where the interactive-command scan belongs when it is written.
{% endraw %}
