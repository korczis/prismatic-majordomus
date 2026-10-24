---
schema: knowledge/v1
id: no-vendor-api-reads-a-chatgpt-project
kind: knowledge
class: constraint
title: "No vendor API reads a person's ChatGPT Project; only the person's own session and the vendor's export reach it"
description: "Applies to any plan for reading or syncing ChatGPT workspace content."
status: verified
epistemics: observed
date: 2026-10-22
tags:
  - providers-economics
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
    - file:.ai/repo/workspaces/chatgpt-majordomus.yaml
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0032-an-external-workspace-is-not-a-provider-the-term-the-depende.md
  - type: documents
    target: file:.ai/repo/adrs/0212-a-workspace-is-read-in-the-operator-s-own-signed-in-browser-or-f.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3279.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3278.yaml
---

# No vendor API reads a person's ChatGPT Project; only the person's own session and the vendor's export reach it

Checked 2026-09-10 and again in a 2026-10-15 conversation: the Conversations API holds only conversations the API created, Sign in with ChatGPT does not expose Projects, and ChatGPT's MCP support calls servers rather than serving history. A Project is a 'gizmo' on the undocumented backend. That review established the operator's own browser session and the vendor's export as acquisition paths; both are read-only, and neither establishes a public Project-sync contract.

## First-party conversation reads have their own support limit

On 2026-10-22, the Codex desktop `read_thread` tool returned accessible ChatGPT conversation records under the operator's own account. This is an observed first-party app capability, not evidence of a public API for enumerating and synchronising a whole Project. Record the actual acquisition mechanism and its support level under I3278/I1302; do not label an app-tool read as a browser capture, an official export or a complete Project sync. Some returned assistant bodies were unresolved content references, so successful retrieval alone did not establish readable content.

## Native subscription access is a separate boundary

On 2026-10-22, a bounded local probe observed Codex authenticated through ChatGPT and Claude Code authenticated through its own subscription. Codex app-server `account/read` returned the ChatGPT authentication type. This proves local account availability only: no model turn, workspace acquisition, usage notification or completed execution was demonstrated by that probe. Authentication is time-bound evidence, not a promise of future quota or availability.

Use the existing native-client seams (`share/advisors.yaml`, I3287 and I3290), with the client's own authentication. A subscription client does not expose the operator's ChatGPT Project history. Do not copy its credentials into MCP, a public runtime or a new gateway; do not silently fall back to a separately billed API. Subscription allowances, observed token usage, estimates and billed amounts remain distinct. I3285 owns the remote read boundary and I3286 owns the still-proposed remote mutation path; local loopback coordination proves neither.

The 2026-10-22 source review also found an acquisition coverage limit: I3278 records 297 indexed conversations but only 234 stored in the 2026-10-18 capture. A listed conversation, a successful tool read and readable message text are different observations. Missing bodies and unresolved content references must remain explicit gaps; neither is evidence that a message was empty. The earlier promotion summary's statement that all 297 were read in full must not be used as coverage proof. Preserve upstream identifiers when supplied, refuse ambiguous positional citations (I3284), and reconcile new decisions against the existing plan, rather than importing another plan beside it.

## Sources

- `chatgpt-majordomus:c267#2`

- [Codex authentication](https://developers.openai.com/codex/auth)
- [Codex app-server](https://developers.openai.com/codex/app-server)
- [Claude Code programmatic use](https://code.claude.com/docs/en/headless)
- [Claude Agent SDK with a Claude plan](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan)
- `file:.ai/repo/project/issues/I3278.yaml`
- `file:.ai/repo/project/issues/I3284.yaml`
- `file:.ai/repo/project/issues/I3285.yaml`
- `file:.ai/repo/project/issues/I3286.yaml`
- `file:.ai/repo/project/issues/I3287.yaml`
- `file:.ai/repo/project/issues/I3290.yaml`
