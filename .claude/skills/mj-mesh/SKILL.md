---
name: mj-mesh
description: "Majordomus mesh: Claim a scope across the mesh; Cooperation, at a glance; The mesh self-check; The cooperation journal; The host firewall, as the mesh needs it; Consume a handover from the mesh; Publish a handover to the mesh; This machine's node identity; The discovered nodes; One runtime; Who…"
argument-hint: "[request]"
---
<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: module:mesh; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.27.0 -->

# Majordomus `mesh`

The capabilities the Majordomus registry declares in its module `mesh`. Choose the one that answers the request below — or, without one, the person's last request about `mesh` — call it through its MCP tool, and report what it answers, as it answered:

$ARGUMENTS

| Capability | What it is | MCP tool | Command line | Effect |
|---|---|---|---|---|
| `mesh.claim` | Claim a scope across the mesh | `majordomus_mesh_claim` | — | changes |
| `mesh.cooperation` | Cooperation, at a glance | `majordomus_mesh_cooperation` | — | reads |
| `mesh.doctor` | The mesh self-check | `majordomus_mesh_doctor` | `majordomus mesh doctor` | reads |
| `mesh.events` | The cooperation journal | `majordomus_mesh_events` | — | reads |
| `mesh.firewall` | The host firewall, as the mesh needs it | `majordomus_mesh_firewall` | `majordomus mesh firewall` | reads |
| `mesh.handover.consume` | Consume a handover from the mesh | `majordomus_mesh_handover_consume` | — | changes |
| `mesh.handover.publish` | Publish a handover to the mesh | `majordomus_mesh_handover_publish` | — | changes |
| `mesh.identity` | This machine's node identity | `majordomus_mesh_identity` | `majordomus mesh identity` | reads |
| `mesh.nodes` | The discovered nodes | `majordomus_mesh_nodes` | — | reads |
| `mesh.peer` | One runtime | `majordomus_mesh_peer` | — | reads |
| `mesh.peers` | Who cooperates, machine by machine | `majordomus_mesh_peers` | — | reads |
| `mesh.register` | Register with this node's mesh | `majordomus_mesh_register` | — | changes |
| `mesh.release` | Release a claim | `majordomus_mesh_release` | — | changes |
| `mesh.review.answer` | Answer a review request | `majordomus_mesh_review_answer` | — | changes |
| `mesh.review.request` | Ask the mesh for a review | `majordomus_mesh_review_request` | — | changes |
| `mesh.review.withdraw` | Withdraw a review request | `majordomus_mesh_review_withdraw` | — | changes |
| `mesh.session.close` | Close a session on the mesh | `majordomus_mesh_session_close` | — | changes |
| `mesh.session.open` | Open or update a session on the mesh | `majordomus_mesh_session_open` | — | changes |
| `mesh.state` | The state every linked runtime converges on | `majordomus_mesh_state` | — | reads |
| `mesh.status` | The mesh, at a glance | `majordomus_mesh` | — | reads |
| `mesh.verify` | Prove cooperation now | `majordomus_mesh_verify` | — | changes |

A capability that *changes* something runs only when the person asked for that change: call it once, and if it is refused or its outcome is not known, report that as it is and do not try again by another way. One that *reads* may be called whenever the request needs it.

Without the Majordomus MCP server attached, run the command line form from the repository root; `--help` shows its arguments. What each capability accepts and answers is defined once, in its declaration: `majordomus capabilities describe <id>`, or the MCP tool's own description. This file is derived from the registry and says nothing the declarations do not.
