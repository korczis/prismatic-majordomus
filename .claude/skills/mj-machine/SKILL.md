---
name: mj-machine
description: "Majordomus machine: Take a machine slot for heavy work; Give a machine slot back; Who holds the machine's slots, and who waits."
argument-hint: "[request]"
---
<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: module:machine; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.10 -->

# Majordomus `machine`

The capabilities the Majordomus registry declares in its module `machine`. Choose the one that answers the request below — or, without one, the person's last request about `machine` — call it through its MCP tool, and report what it answers, as it answered:

$ARGUMENTS

| Capability | What it is | MCP tool | Command line | Effect |
|---|---|---|---|---|
| `machine.slot.acquire` | Take a machine slot for heavy work | `majordomus_machine_slot_acquire` | `majordomus machine slot acquire` | changes |
| `machine.slot.release` | Give a machine slot back | `majordomus_machine_slot_release` | `majordomus machine slot release` | changes |
| `machine.slot.status` | Who holds the machine's slots, and who waits | `majordomus_machine_slot_status` | `majordomus machine slot status` | reads |

A capability that *changes* something runs only when the person asked for that change: call it once, and if it is refused or its outcome is not known, report that as it is and do not try again by another way. One that *reads* may be called whenever the request needs it.

Without the Majordomus MCP server attached, run the command line form from the repository root; `--help` shows its arguments. What each capability accepts and answers is defined once, in its declaration: `majordomus capabilities describe <id>`, or the MCP tool's own description. This file is derived from the registry and says nothing the declarations do not.
