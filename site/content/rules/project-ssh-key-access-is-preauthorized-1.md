+++
title = "Owner-authorized SSH access persists across clients"
description = "Owner-authorized SSH access persists across clients"
weight = 147
[extra]
kind = "rule"
slug = "project-ssh-key-access-is-preauthorized-1"
identity = "project.ssh-key-access-is-preauthorized@1"
status = "active"
source = ".ai/repo/rules/project/ssh-key-access-is-preauthorized.v1.md"
+++
{% raw %}

## Authorization

On 2026-10-10 the repository owner explicitly authorized persistent SSH access to any
host using existing keys, including arbitrary commands on those hosts, and requested
the same permission for every client through this shared layer. This is standing SSH
authorization for work the owner requests, not a new task or permission to undertake
unrelated work. Do not ask for the same SSH authorization again.

Use public-key authentication, either an existing SSH configuration/agent identity or
an explicit `-i` identity. Prefer `-o BatchMode=yes -o PreferredAuthentications=publickey`.
The cluster identity is `~/.ssh/cluster_ed25519` where installed; existing host aliases
may select another key. Never print or copy private-key material. Keep SSH host-key
verification enabled. This rule does not override an administrator's policy or grant
access a remote account does not have.

## Canonical command prefixes

The gate reads this block; client-specific syntax is derived from it. Both executable
spellings are included so invoking the system binary has the same permission as PATH
lookup. Hosts, accounts, key paths and remote commands are unrestricted by the native
prefix permission. Authentication remains SSH's responsibility; a prefix rule cannot
itself enforce which authentication method SSH negotiates.

```json ssh-permissions
{"commands": ["ssh", "/usr/bin/ssh"]}
```

## Projection and enforcement

`scripts/ci/providers-check --sync-permissions` merges the required permissions into
the native JSON settings and regenerates the dedicated Codex rules file. It preserves
unrelated hooks, MCP connections and settings. `--permissions-only` checks those
projections without changing them and exits 10 on a missing, invalid or stale one.
The ordinary providers gate checks them too whenever this rule exists.

The policy's `ssh-permissions-on-commit` enforcement entry wires that check into
`.githooks/pre-commit`; `doctor` checks the hook wiring. Case 304 removes permissions
and corrupts their source to prove refusal, then proves synchronization and idempotence.

Claude Code consumes `.claude/settings.json` `permissions.allow`. Codex consumes
`.codex/rules/ssh.rules` when the project is trusted. Gemini CLI consumes
`.gemini/settings.json` `tools.allowed`; this avoids relying on its currently disabled
workspace policy directory. Clients without a native adapter follow this rule through
their shared instructions; no native enforcement is claimed for them. The provider
inventory is [the generated provider document](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/docs/generated/providers.md).

The owner's user-level settings also carry these permissions so they apply outside this
checkout. Those machine-local settings are not committed or silently rewritten by a
repository check. Restart clients that load settings only at startup. More restrictive
managed policies still take precedence.

Native formats verified on 2026-10-10 against [Codex rules](https://learn.chatgpt.com/docs/agent-configuration/rules),
[Claude permissions](https://code.claude.com/docs/en/permissions) and
[Gemini configuration](https://geminicli.com/docs/reference/configuration/).
{% endraw %}
