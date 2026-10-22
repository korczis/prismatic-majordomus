+++
title = "Autonomy"
description = "how much a coding-agent session may do unasked, read from the facts: the four profiles and the Claude Code mode each launches with, why selection fails closed (a checked-in setting raises nothing, a worktree is no boundary, a credential in reach refuses isolation), the per-command decision and its exit codes, and the measured cost of the permission layer"
weight = 48
[extra]
source = "docs/AUTONOMY.md"
+++

{% raw %}

How much a coding-agent session in a checkout may do without a person's approval, read
from the facts rather than assumed (I2790). Four read-only capabilities answer it. None
writes a setting; the one command that does, `autonomy apply`, writes only the owner's
declared approvals and is run by a person. Granting autonomy is the person's act, and a session that judged its own
autonomy and then granted it would be exactly what the profile exists to prevent.

<div class="overflow-x-auto" tabindex="0">

| Capability | CLI | MCP | HTTP |
| --- | --- | --- | --- |
| `autonomy.status` | `majordomus autonomy status` | `majordomus_autonomy` | `GET /api/v1/autonomy` |
| `autonomy.explain` | `majordomus autonomy explain "<command>"` | `majordomus_autonomy_explain` | `GET /api/v1/autonomy/explain` |
| `autonomy.friction` | `majordomus autonomy friction` | `majordomus_autonomy_friction` | `GET /api/v1/autonomy/friction` |

</div>


## Profiles

<div class="overflow-x-auto" tabindex="0">

| Profile | Claude Code mode | Requires |
| --- | --- | --- |
| `conservative` | `default` | nothing; also the ceiling when a settings layer cannot be read or the managed settings set `default` |
| `balanced` | `acceptEdits` | every settings layer reads |
| `trusted` | `auto` | the person accepted this checkout in the client's trust dialog, read from their own `~/.claude.json` |
| `isolated` | `bypassPermissions` | a container marker (`/.dockerenv`, `/run/.containerenv`, the `container` variable), no credential variable set and no credential file in reach, and no layer disabling the bypass |

</div>


Selection fails closed:

- A checked-in `.claude/settings.json` that asks for `bypassPermissions` raises nothing. It
  is the repository's own word, and an untrusted repository must not be able to declare
  itself privileged.
- A git worktree is not a security boundary, and the absence of a container marker is not
  proof that there is no container. `isolated` also requires that no credential be in reach:
  `GITHUB_TOKEN`, `GH_TOKEN`, the AWS and Azure variables, `SSH_AUTH_SOCK`, and `~/.ssh/id_*`,
  `~/.config/gh/hosts.yml`, `~/.aws/credentials`, `~/.netrc` and the others named in
  `src/autonomy/mod.rs`. Variables are reported by name, never by value.
- An unreadable settings layer drops the profile to `conservative`, because the client may
  be enforcing rules nobody can see. A managed `defaultMode` is the organisation's ceiling.

`status` reports each layer (managed, local, project, user) with its presence, its default
mode, its rule counts and the hook events it wires, but never the rule text, because a rule
may name a host, a path or a person. Every stronger profile appears under `refused`, with
the fact that refused it.

## One command

`autonomy explain` gives one shell command a structural decision. The exit code carries it:

<div class="overflow-x-auto" tabindex="0">

| Exit | Decision | Meaning |
| --- | --- | --- |
| 0 | `allow` | a static allow rule may run it unasked under the profile |
| 10 | `defer` | no static rule decides it; the client's own classifier or prompt does |
| 11 | `require_approval` | a person approves it, under every profile, `isolated` included |

</div>


The table reads the boundaries first, so `git push --force; ls` is named for the push and
not only for being two commands. History rewrites (`git reset --hard`, `git clean`,
`git branch -D`, `git commit --amend`), removal (`rm`, `find -delete`), privilege (`sudo`,
`kill`) and external effects (`git push`, `gh pr merge`, `curl`, `ssh`) require approval
under every profile. A compound command never earns an allow. That covers chaining, pipes,
redirection, `$(...)` and backticks (inside double quotes as well), a leading variable
assignment, `find -exec` and an unclosed quote. A prefix rule sees only the first word, and
the rest of the line can be anything.

## What the permission layer cost

`autonomy friction --since-days N` reads Claude Code's own transcripts of this repository:
the primary checkout and every `<repository>-wt/<branch>` worktree under
`~/.claude/projects`. It counts tool calls and every refusal: by kind (`classifier_denial`,
`hook_block`, `safety_check`, `user_rejection`), by the classifier's stated reason and by
tool, with the rate per thousand calls. It counts only, so no command, path or payload
leaves a transcript. A prompt the person approved leaves no trace and is not counted.

The baseline that motivated this was measured across 102 transcripts and 33,345 tool calls,
2026-10-09 to 10-16: 110 classifier denials, 31 hook blocks and 15 rejected prompts. None of
them fell on routine repository work. Every denial was a boundary class: interfering with
workloads (28), self-modification of settings (17), merging without review (8), creating
unsafe agents (6), CI bypass (5), modifying shared resources (5) and irreversible local
destruction (4). The friction left on this host sits at the boundaries the profiles keep,
not in reads, builds and edits.

## Standing approvals on every machine

The owner's standing approvals are declared once, in `.ai/repo/autonomy/claude-code.yaml`
(kind `autonomy-declaration`, I3900). Each one has a date and the rule exactly as the
auto-mode classifier reads it, with the owner's words quoted inside. The declaration also
carries the environment lines the classifier is told and `crossSessionInbound`. Claude Code
reads permissions from each machine's own `~/.claude/settings.json`, so before this an
approval was held only on the machine where it was given. On 2026-10-20 one MacBook asked
for eight approvals that the other MacBook had held for a week.

<div class="overflow-x-auto" tabindex="0">

| Capability | CLI | MCP | HTTP |
| --- | --- | --- | --- |
| `autonomy.approvals` | `majordomus autonomy approvals` | `majordomus_autonomy_approvals` | `GET /api/v1/autonomy/approvals` |

</div>


The verdict is one of five values:

- `full`: every declared approval, environment line and setting is held, and `$defaults`
  stays first.
- `partial` or `none`: some or all of them are missing. The answer names
  `majordomus autonomy apply` as the remedy.
- `undeclared`: the repository declares nothing.
- `unreadable`: the settings file cannot be parsed. Nothing is judged and nothing is
  written.

An approval counts as held only when its rule is an exact entry of `autoMode.allow`. A rule
that was edited on the machine is not held. Entries the declaration does not name are
counted but never quoted. The brief adds an `autonomy` line on every machine, and adds
`approvals n/m` to its one line on a machine that lacks something.

`majordomus autonomy apply` writes into the user settings only what is declared and missing:

- `$defaults` goes first.
- Then each missing rule and environment line is appended, in declaration order.
- The declared settings are set.

Apply keeps every other key, entry and hook, and keeps a copy of the file as it was
(`settings.json.bak-majordomus-<stamp>`). Run on a machine that already holds everything,
it writes nothing. It is a command the person runs. No projection offers it.

An approval is a grant, so it reaches a machine only once the owner's trunk holds it. The
working tree's declaration is narrowed to what the trunk's committed copy also declares
identically, the way `mesh doctor`'s trust root narrows the mesh declaration. A branch, a
pull request or an uncommitted edit can withdraw an approval but never add one. A
repository whose trunk holds no copy grants nothing. Every item that was narrowed away is
listed under `narrowed`. `apply` also refuses to run inside an agent session (when
`CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, `CODEX_SANDBOX` or `GEMINI_CLI` is set), because a
session that wrote its own permissions would be granting itself what only the person grants.

A worker that writes a new standing approval adds it to the declaration in the same change
(`project.a-standing-approval-is-written-where-permissions-are-read`). The declaration
cannot raise the permission mode, add an allow prefix or touch a hook. The repository is
public, so nothing secret belongs in it.

## Limits

- The running session's effective permission mode belongs to the client, set by a
  `--permission-mode` flag or switched mid-session, and is not observable from a child
  process. `configured_mode` is what the settings say.
- Nothing here launches a session or writes the derived permission rules. A launcher that
  starts Claude Code with the recommended mode, a generated project allow-list and a
  `PreToolUse` hook on Bash that asks `autonomy.explain` are follow-up work.
{% endraw %}
