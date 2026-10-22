---
name: mj-autonomy
description: "Majordomus autonomy: Whether this machine holds the owner's standing approvals; Judge one shell command; What the permission layer cost; The autonomy this checkout supports."
argument-hint: "[request]"
---
<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: module:autonomy; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.6 -->

# Majordomus `autonomy`

The capabilities the Majordomus registry declares in its module `autonomy`. Choose the one that answers the request below — or, without one, the person's last request about `autonomy` — call it through its MCP tool, and report what it answers, as it answered:

$ARGUMENTS

| Capability | What it is | MCP tool | Command line | Effect |
|---|---|---|---|---|
| `autonomy.approvals` | Whether this machine holds the owner's standing approvals | `majordomus_autonomy_approvals` | `majordomus autonomy approvals` | reads |
| `autonomy.explain` | Judge one shell command | `majordomus_autonomy_explain` | `majordomus autonomy explain` | reads |
| `autonomy.friction` | What the permission layer cost | `majordomus_autonomy_friction` | `majordomus autonomy friction` | reads |
| `autonomy.status` | The autonomy this checkout supports | `majordomus_autonomy` | `majordomus autonomy status` | reads |

A capability that *changes* something runs only when the person asked for that change: call it once, and if it is refused or its outcome is not known, report that as it is and do not try again by another way. One that *reads* may be called whenever the request needs it.

Without the Majordomus MCP server attached, run the command line form from the repository root; `--help` shows its arguments. What each capability accepts and answers is defined once, in its declaration: `majordomus capabilities describe <id>`, or the MCP tool's own description. This file is derived from the registry and says nothing the declarations do not.
