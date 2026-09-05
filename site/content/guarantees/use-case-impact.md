+++
title = "From the files a change touched, the tool names the commands, rules, use cases, scenarios and behavioural cases affected, and scaffolds a draft for a capability no use case covers"
description = "A worker who changed a file does not have to remember which use cases describe it."
weight = 126
[extra]
claim_id = "use-case-impact"
status = "guaranteed"
source = "docs/claims/use-case-impact.md"
+++
{% raw %}

## What it means

A worker who changed a file does not have to remember which use cases describe it.
`majordomus usecase impact` reads the files changed since a base (the upstream by
default) and in the work tree and names the commands, the rules, the use cases, the
scenarios among them and the behavioural cases they reach, ending with the run to do.
`majordomus usecase scaffold` writes a draft for a capability no active use case names,
from what the registry, the command's fixture and the claims already know, and never
marks anything guaranteed.

## How it works

`mj_uc_cmd_impact` maps paths: `lib/<command>.sh` to the command, a responsibility's
`files` and `implementation` (`docs/RESPONSIBILITIES.yaml`) to its command, a rule file to
its front-matter id, a use-case file, setup script or stdin body to the use cases that use
it, `docs/CLAIMS.yaml` to every use case naming a claim, the crate to every use case
naming an MCP tool, and `bin/majordomus`, `lib/common.sh`, the registry, the manifest and
the policy to every command; commands and rules then map to the use cases naming them,
and to the cases whose `majordomus-covers` header names the command or which a rule lists
under `tests`. `mj_uc_cmd_scaffold` writes `<command>-draft.md` with `status: draft`,
`target: advisory` and a scenario taken from the command's fixture.

## How to see it

```bash
majordomus usecase impact --base origin/master
majordomus usecase impact --json
majordomus usecase scaffold --missing --dry-run
```

## What it does not cover

Impact is computed from declared relations, not from a call graph: a helper used by every
command is reached only when it lives in a file the map knows (`lib/common.sh` reaches
everything). A draft is a starting point; the narrative and the assertions are a
person's or an agent's work.

## Why it exists

The maintenance graph of a capability is too large to remember. The tool remembers it,
and says what to run.
{% endraw %}
